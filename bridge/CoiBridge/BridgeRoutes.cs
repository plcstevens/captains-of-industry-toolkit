using System;
using System.Collections.Generic;
using System.Text;
using Mafi;

// VERIFIED 2026-11-02 against Philip's shipped game assemblies (Mafi.Core.dll,
// game 0.8.8): the web API lives in Mafi.Core.WebApi. WebRouteAttribute's
// constructor is (string path, WebRouteStage stage, string method = "GET",
// string description = ""); handlers return WebRouteResponse (factories:
// .Json/.Text/.Html/.Bytes, optional status). The game's own DemoDashboard
// class is the reference shape: a global dependency whose [WebRoute] methods
// are private and return WebRouteResponse. WebApiServer.scanAllRoutes()
// registers the [WebRoute] methods of every instance in the resolver's
// AllResolvedInstances — CoiBridgeMod.Initialize resolves this class, which
// is what puts it in that set.
//
// M2 (2026-11-02 evening): the data routes are wired to the real manager
// surfaces, all read from the shipped assemblies (reflection dumps in the
// build notes): IEntitiesManager.Entities / ResearchManager /
// TrainLinesManager.Lines / SettlementsManager. Value-type facts used:
// EntityId.Value (int), Quantity.Value (int), Percent.RawValue (int32,
// presumably per-10000), Proto.Id.Value (string), Tile3i X/Y/Z fields,
// IoPort.OwnerEntity (IEntityWithPorts) + ConnectedPort.
using Mafi.Core.Buildings.Settlements;
using Mafi.Core.Buildings.Storages;
using Mafi.Core.Entities;
using Mafi.Core.Entities.Priorities;
using Mafi.Core.Entities.Static;
using Mafi.Core.Entities.Static.Layout;
using Mafi.Core.Factory;
using Mafi.Core.Factory.Machines;
using Mafi.Core.Factory.Transports;
using Mafi.Core.Ports.Io;
using Mafi.Core.Research;
using Mafi.Core.Trains;
using Mafi.Core.WebApi;

namespace CoiBridge;

/// <summary>
/// HTTP route handlers for the CoI Companion bridge, served by the game's built-in
/// web server (started in-game with the `web_api_start` console command) alongside
/// the vanilla routes. Registered as a global dependency two ways — the
/// [GlobalDependency] attribute below and CoiBridgeMod's explicit
/// depBuilder.RegisterDependency call (the shipped PriorityManager mod does
/// both; see bridge/MODDING-NOTES.md).
///
/// Routes: hello + debug/services (M1) and the M2 data routes — research,
/// census, topology, storages, status, trains — each backed by a verified
/// manager surface and wrapped so a failure becomes a JSON 500, never an
/// exception into the sim.
///
/// Hard rules for every handler (SPEC sections 3 and 10):
///   * read-only — no setters, no state changes, ever (Tier 1);
///   * census/topology scans run on the Background stage and are cached
///     ~5 s; manager aggregates (research/status/trains) are cheap singles;
///   * every entity record carries {id, proto, pos} — the shape §12 needs.
/// </summary>
[GlobalDependency(RegistrationMode.AsEverything)]
public sealed class BridgeRoutes {

	/// <summary>Mod version reported by the routes. Keep in step with manifest.json.</summary>
	public const string ModVersion = "0.6.4";

	/// <summary>Route namespace version (SPEC section 3). Breaking changes bump this.</summary>
	public const string GameApi = "v1";

	private static readonly string[] s_routeTable = {
		"GET /coi/v1/hello",
		"GET /coi/v1/debug/services",
		"GET /coi/v1/research",
		"GET /coi/v1/trains",
		"GET /coi/v1/census",
		"GET /coi/v1/topology",
		"GET /coi/v1/storages",
		"GET /coi/v1/status",
		"GET /coi/v1/contracts",
		"GET /coi/v1/loans",
		"GET /coi/v1/edicts",
		"GET /coi/v1/trains-detail",
		"GET /coi/v1/quick-trades",
		"GET /coi/v1/power",
		"GET /coi/v1/priorities",
		"GET /coi/v1/machines",
	};

	private readonly IEntitiesManager m_entities;
	private readonly ResearchManager m_research;
	private readonly SettlementsManager m_settlements;
	private readonly TrainLinesManager m_trainLines;
	private readonly TrainsManager m_trainsMgr;

	private readonly object m_scanLock = new object();
	private string m_censusJson;
	private string m_topologyJson;
	private DateTime m_scanUtc = DateTime.MinValue;
	private const double ScanTtlSeconds = 5.0;

	/// <summary>
	/// Constructor injection of the verified manager surfaces. All four are
	/// global dependencies in the shipped game (ResearchManager is
	/// constructor-injected by the shipped ResearchQueue mod the same way).
	/// </summary>
	public BridgeRoutes(IEntitiesManager entities, ResearchManager research,
			SettlementsManager settlements, TrainLinesManager trainLines,
			TrainsManager trainsMgr) {
		m_entities = entities;
		m_research = research;
		m_settlements = settlements;
		m_trainLines = trainLines;
		m_trainsMgr = trainsMgr;
		Log.Info("CoiBridge: BridgeRoutes constructed (M2 managers injected)");
	}

	/// <summary>Liveness probe. SPEC section 9, milestone M1 acceptance route.</summary>
	[WebRoute("/coi/v1/hello", WebRouteStage.Background, "GET", "CoiBridge liveness probe: mod name, version, route namespace.")]
	public WebRouteResponse Hello() {
		return WebRouteResponse.Json(
			"{\"mod\":\"CoiBridge\",\"version\":\"" + ModVersion + "\",\"gameApi\":\"" + GameApi + "\"}");
	}

	/// <summary>Discovery endpoint (SPEC section 3): route table + reflection metadata. Metadata only; never invokes game methods.</summary>
	[WebRoute("/coi/v1/debug/services", WebRouteStage.Background, "GET", "CoiBridge runtime discovery report (reflection metadata only).")]
	public WebRouteResponse DebugServices() {
		return WebRouteResponse.Json(DiscoveryReport.Build(s_routeTable, ModVersion, GameApi));
	}

	/// <summary>
	/// Research state: current node + progress, queue (in order), the
	/// completed (researched) node ids, researched count, and whether
	/// any lab is active. Backing: ResearchManager
	/// (CurrentResearch : Option&lt;ResearchNode&gt;, ResearchQueue,
	/// ResearchedNodes, HasActiveLab — all verified public surfaces).
	/// The id list (v0.6.2) closes the game-plan format's G1 gap: gates
	/// can now test research completion against live state instead of
	/// executor-observed history.
	/// </summary>
	[WebRoute("/coi/v1/research", WebRouteStage.Background, "GET", "Research state: current node, progress, queue, researched ids and count.")]
	public WebRouteResponse Research() {
		return Guard("/coi/v1/research", () => {
			var sb = new StringBuilder();
			sb.Append("{\"current\":");
			Option<ResearchNode> cur = m_research.CurrentResearch;
			if (cur.HasValue) {
				ResearchNode n = cur.Value;
				sb.Append("{\"id\":\"").Append(JsonEscape(n.Proto.Id.Value))
					.Append("\",\"progressPerc\":").Append(n.ProgressInPerc.RawValue)
					.Append(",\"stepsDone\":").Append(n.StepsDone)
					.Append(",\"remainingSteps\":").Append(n.RemainingSteps)
					.Append(",\"scienceCost\":").Append(n.ScienceCost)
					.Append('}');
			} else {
				sb.Append("null");
			}
			sb.Append(",\"queue\":[");
			bool first = true;
			foreach (ResearchNode q in m_research.ResearchQueue) {
				if (!first) sb.Append(',');
				first = false;
				sb.Append('"').Append(JsonEscape(q.Proto.Id.Value)).Append('"');
			}
			sb.Append("],\"researched\":[");
			first = true;
			foreach (ResearchNode r in m_research.ResearchedNodes) {
				if (!first) sb.Append(',');
				first = false;
				sb.Append('"').Append(JsonEscape(r.Proto.Id.Value)).Append('"');
			}
			sb.Append("],\"researchedCount\":").Append(m_research.ResearchedNodes.Count)
				.Append(",\"hasActiveLab\":").Append(m_research.HasActiveLab ? "true" : "false")
				.Append('}');
			return sb.ToString();
		});
	}

	/// <summary>
	/// Settlement headline numbers — the gap in vanilla game-info:
	/// population, housing, food months, starving flags, free workers.
	/// Backing: SettlementsManager (verified public surfaces).
	/// </summary>
	[WebRoute("/coi/v1/status", WebRouteStage.Background, "GET", "Population and settlement headline numbers.")]
	public WebRouteResponse Status() {
		return Guard("/coi/v1/status", () => {
			var sb = new StringBuilder();
			int population = 0;
			foreach (Settlement s in m_settlements.Settlements) {
				population += s.Population;
			}
			sb.Append("{\"population\":").Append(population)
				.Append(",\"totalHousingCapacity\":").Append(m_settlements.TotalHousingCapacity)
				.Append(",\"freeHousingCapacity\":").Append(m_settlements.FreeHousingCapacity)
				.Append(",\"numberOfHomeless\":").Append(m_settlements.NumberOfHomeless)
				.Append(",\"monthsOfFood\":").Append(m_settlements.MonthsOfFood)
				.Append(",\"arePeopleStarving\":").Append(m_settlements.ArePeopleStarving ? "true" : "false")
				.Append(",\"settlements\":").Append(m_settlements.SettlementsCount)
				.Append('}');
			return sb.ToString();
		});
	}

	/// <summary>
	/// Train lines: name, train/station counts per line. Per-train activity
	/// and travel statistics (ProductivityCounterHistory surfaces on Train)
	/// are deliberately NOT decoded yet — their counter semantics need a
	/// live cross-check against the in-game train stats panel first.
	/// </summary>
	[WebRoute("/coi/v1/trains", WebRouteStage.Background, "GET", "Train lines: names, train and station counts.")]
	public WebRouteResponse Trains() {
		return Guard("/coi/v1/trains", () => {
			var sb = new StringBuilder();
			sb.Append("{\"lines\":[");
			bool first = true;
			foreach (TrainLine line in m_trainLines.Lines) {
				if (!first) sb.Append(',');
				first = false;
				sb.Append("{\"name\":\"").Append(JsonEscape(line.Name.ToString()))
					.Append("\",\"trains\":").Append(line.TrainsCount)
					.Append(",\"stations\":").Append(line.StationsCount)
					.Append('}');
			}
			sb.Append(']');
			// v0.6.4: RAW per-train productivity counters. The values
			// are the game's ProductivityCounter buckets exactly as
			// stored (recon round 4 in MODDING-NOTES.md) — category
			// report counts per 600-report bucket, NOT percentages.
			// The decoding (which denominator the in-game panel uses,
			// ring order, the Version==0 ongoing-ticks quirk) is NOT
			// live-verified, hence "unverifiedDecoding": true at the
			// top level of this payload. The counters block is built
			// behind its own try/catch so a counter failure can never
			// take the lines payload down with it.
			sb.Append(",\"unverifiedDecoding\":true");
			sb.Append(",\"counterLabels\":{")
				.Append("\"activity\":{\"a\":\"Working\",\"b\":\"Idle\",\"c\":\"Refueling\",\"d\":\"CannotWork\",\"noCategory\":\"Paused\"},")
				.Append("\"travel\":{\"a\":\"FullSpeed\",\"b\":\"AcceleratingAndBraking\",\"c\":\"TrackSpeedLimit\",\"d\":\"BlockedByTrains\",\"noCategory\":\"NotTraveling\"}}");
			try {
				sb.Append(",\"trains\":[");
				bool ft = true;
				foreach (Train t in m_trainsMgr.Trains) {
					if (!ft) sb.Append(',');
					ft = false;
					sb.Append("{\"trainId\":").Append(t.TrainId.Value)
						.Append(",\"name\":\"").Append(JsonEscape(t.Name ?? ""))
						.Append("\",\"line\":");
					if (t.TrainLine.HasValue) {
						TrainLine tl = t.TrainLine.Value;
						sb.Append("{\"id\":").Append(tl.Id.Value)
							.Append(",\"name\":\"").Append(JsonEscape(tl.Name.ToString()))
							.Append("\"}");
					} else {
						sb.Append("null");
					}
					sb.Append(",\"isPaused\":").Append(t.IsPaused ? "true" : "false")
						.Append(",\"isSpawned\":").Append(t.IsSpawned ? "true" : "false")
						.Append(",\"cars\":").Append(t.TrainCarsCount)
						.Append(",\"activity\":");
					AppendCounterHistory(sb, t.ActivityStats, t.ActivityStatsOngoingMonth, t.ActivityStatsOngoingMonthTicks);
					sb.Append(",\"travel\":");
					AppendCounterHistory(sb, t.TravelStats, t.TravelStatsOngoingMonth, t.TravelStatsOngoingMonthTicks);
					sb.Append('}');
				}
				sb.Append(']');
			} catch (Exception ex) {
				Log.Error("CoiBridge: /coi/v1/trains counters block failed: " + ex);
				sb.Append(",\"trains\":[],\"trainsError\":\"")
					.Append(JsonEscape(ex.GetType().Name)).Append('"');
			}
			sb.Append('}');
			return sb.ToString();
		});
	}

	/// <summary>One ProductivityCounterHistory.Data bucket, raw:
	/// the four category report counts, their sum, and the packed
	/// 64-bit union value exactly as stored.</summary>
	private static void AppendCounterData(StringBuilder sb, ProductivityCounterHistory.Data d) {
		sb.Append("{\"a\":").Append(d.CategoryA)
			.Append(",\"b\":").Append(d.CategoryB)
			.Append(",\"c\":").Append(d.CategoryC)
			.Append(",\"d\":").Append(d.CategoryD)
			.Append(",\"total\":").Append(d.Total)
			.Append(",\"rawData\":").Append(d.RawData)
			.Append('}');
	}

	/// <summary>A full ProductivityCounterHistory, raw: version
	/// counters, the monthly ring (stored order — the ring index
	/// points at the newest slot per the recon; consumers must not
	/// assume chronological order), yearly rolling + ring, the
	/// halved lifetime rolling, and the ongoing (partial) bucket.</summary>
	private static void AppendCounterHistory(StringBuilder sb,
			ProductivityCounterHistory h,
			ProductivityCounterHistory.Data ongoing, int ongoingTicks) {
		sb.Append("{\"version\":").Append(h.Version)
			.Append(",\"fullMonthlyRecordsCount\":").Append(h.FullMonthlyRecordsCount)
			.Append(",\"yearlyRecordsCount\":").Append(h.YearlyRecordsCount)
			.Append(",\"monthlyRecordsBufferIndex\":").Append(h.MonthlyRecordsBufferIndex)
			.Append(",\"monthlyRecords\":[");
		bool first = true;
		foreach (ProductivityCounterHistory.Data d in h.MonthlyRecordsBuffer) {
			if (!first) sb.Append(',');
			first = false;
			AppendCounterData(sb, d);
		}
		sb.Append("],\"yearlyRecordsBufferIndex\":").Append(h.YearlyRecordsBufferIndex)
			.Append(",\"yearlyRecords\":[");
		first = true;
		foreach (ProductivityCounterHistory.Data d in h.YearlyRecordsBuffer) {
			if (!first) sb.Append(',');
			first = false;
			AppendCounterData(sb, d);
		}
		sb.Append("],\"yearlyRolling\":");
		AppendCounterData(sb, h.YearlyRolling);
		sb.Append(",\"totalRollingHalf\":");
		AppendCounterData(sb, h.TotalRollingHalf);
		sb.Append(",\"ongoingMonth\":");
		AppendCounterData(sb, ongoing);
		sb.Append(",\"ongoingMonthTicks\":").Append(ongoingTicks)
			.Append('}');
	}

	/// <summary>
	/// Entity census: per-proto counts for every placed entity, machine
	/// state splits (Working / NotEnoughInput / OutputFull / Paused / …)
	/// and recipe assignments, storage fill summaries, transport totals,
	/// and a compact priorities map (v0.6.1: entityId → general priority,
	/// ONLY for entities whose value differs from their proto default —
	/// LayoutEntity.GeneralPriority vs Prototype.Costs.DefaultPriority,
	/// see bridge/MODDING-NOTES.md recon round 2). Cached ~5 s.
	/// </summary>
	[WebRoute("/coi/v1/census", WebRouteStage.Background, "GET", "Entity census: counts by proto, machine states, storage fills, non-default priorities.")]
	public WebRouteResponse Census() {
		return Guard("/coi/v1/census", () => {
			EnsureScans();
			return m_censusJson;
		});
	}

	/// <summary>
	/// Topology: every layout entity as {id, proto, pos:[x,y,z]}, every
	/// transport with start/end tiles and connection flag, and every port
	/// connection as an entity-id edge. This is the graph the design
	/// diagnostics reason over (SPEC §5; Philip's topology ask 2026-11-02).
	/// Cached ~5 s (shared scan with the census).
	/// </summary>
	[WebRoute("/coi/v1/topology", WebRouteStage.Background, "GET", "Entity + transport + port-connection graph.")]
	public WebRouteResponse Topology() {
		return Guard("/coi/v1/topology", () => {
			EnsureScans();
			return m_topologyJson;
		});
	}

	/// <summary>
	/// Per-storage detail: proto, position, assigned product, quantity,
	/// capacity, percent full, logistics in/out disabled flags, general
	/// priority (v0.6.1); concrete Storage rows also carry the custom
	/// import/export priorities and the four logistics slider steps
	/// (derived from the Storage percent properties — recon round 2).
	/// Read off StorageBase entities in the shared scan (not a
	/// separate walk).
	/// </summary>
	[WebRoute("/coi/v1/storages", WebRouteStage.Background, "GET", "Per-storage contents and fill levels.")]
	public WebRouteResponse Storages() {
		return Guard("/coi/v1/storages", () => {
			EnsureScans();
			return m_storagesJson;
		});
	}

	private string m_storagesJson;

	/// <summary>
	/// Per-machine instances (v0.6.4): one row per placed machine —
	/// id, proto, position, live state, assigned recipes, paused /
	/// enabled flags. The census only aggregates states per proto;
	/// this route is what lets a consumer map a state (starved,
	/// output-full) to the actual entities. Rows are built in the
	/// shared scan; a row that fails to build is skipped (counted in
	/// "skippedRows") so a bad machine can never poison the scan the
	/// other routes depend on.
	/// </summary>
	[WebRoute("/coi/v1/machines", WebRouteStage.Background, "GET", "Per-machine instances: state, recipes, paused/enabled per entity.")]
	public WebRouteResponse Machines() {
		return Guard("/coi/v1/machines", () => {
			EnsureScans();
			return m_machinesJson;
		});
	}

	private string m_machinesJson;

	// ------------------------------------------------------------------
	// Shared entity scan (census + topology + storages in one walk)
	// ------------------------------------------------------------------

	private void EnsureScans() {
		lock (m_scanLock) {
			if (m_censusJson != null && (DateTime.UtcNow - m_scanUtc).TotalSeconds < ScanTtlSeconds) {
				return;
			}
			BuildScans();
			m_scanUtc = DateTime.UtcNow;
		}
	}

	private void BuildScans() {
		var census = new StringBuilder();
		var topo = new StringBuilder();
		var stor = new StringBuilder();
		var mach = new StringBuilder();

		var protoCounts = new Dictionary<string, int>();
		var machineStates = new Dictionary<string, Dictionary<string, int>>();
		var machineRecipes = new Dictionary<string, HashSet<string>>();
		var storageRows = new List<string>();
		var priorityRows = new List<string>();
		var machineRows = new List<string>();
		var entityRows = new List<string>();
		var transportRows = new List<string>();
		var edgeRows = new List<string>();
		int totalEntities = 0, transports = 0, connectedTransports = 0;
		int machineRowsSkipped = 0;

		foreach (IEntity e in m_entities.Entities) {
			if (e.IsDestroyed) continue;
			totalEntities++;
			string proto = e.Prototype.Id.Value;
			protoCounts[proto] = protoCounts.TryGetValue(proto, out int c) ? c + 1 : 1;

			// General priority (recon round 2): the value lives on
			// LayoutEntity behind IEntityWithGeneralPriority; the proto
			// default is Prototype.Costs.DefaultPriority. Record ONLY
			// non-default values so the census stays compact.
			if (e is IEntityWithGeneralPriority gp
					&& gp.GeneralPriority != gp.Prototype.Costs.DefaultPriority) {
				priorityRows.Add("\"" + e.Id.Value + "\":" + gp.GeneralPriority);
			}

			if (e is LayoutEntity le) {
				Tile3i ctr = le.CenterTile;
				entityRows.Add("{\"id\":" + e.Id.Value + ",\"proto\":\"" + JsonEscape(proto) +
					"\",\"pos\":[" + ctr.X + "," + ctr.Y + "," + ctr.Z + "]}");

				// Port edges: each connected port pair becomes one edge
				// (owner entity ids), de-duplicated by ordering.
				foreach (IoPort port in le.Ports) {
					if (port.IsConnected && port.ConnectedPort.HasValue) {
						IEntity other = port.ConnectedPort.Value.OwnerEntity as IEntity;
						if (other != null && other.Id.Value > e.Id.Value) {
							edgeRows.Add("[" + e.Id.Value + "," + other.Id.Value + "]");
						}
					}
				}
			}

			if (e is Machine m) {
				string state = m.CurrentState.ToString();
				if (!machineStates.TryGetValue(proto, out Dictionary<string, int> states)) {
					states = new Dictionary<string, int>();
					machineStates[proto] = states;
				}
				states[state] = states.TryGetValue(state, out int sc) ? sc + 1 : 1;
				if (!machineRecipes.TryGetValue(proto, out HashSet<string> recipes)) {
					recipes = new HashSet<string>();
					machineRecipes[proto] = recipes;
				}
				foreach (var rp in m.RecipesAssigned) {
					recipes.Add(rp.Id.Value);
				}
				// v0.6.4 per-instance row for /coi/v1/machines. Own
				// try/catch: a single bad row is skipped, never fatal
				// to the shared scan.
				try {
					machineRows.Add(BuildMachineRow(e.Id.Value, proto, m));
				} catch (Exception) {
					machineRowsSkipped++;
				}
			}

			if (e is StorageBase st) {
				string product = st.StoredProduct.HasValue ? st.StoredProduct.Value.Id.Value : "";
				storageRows.Add(BuildStorageRow(e.Id.Value, proto, product, st));
			}

			if (e is Transport t) {
				transports++;
				if (t.IsFullyConnected) connectedTransports++;
				transportRows.Add(BuildTransportRow(e.Id.Value, proto, t.StartPosition, t.EndPosition, t));
			}
		}

		census.Append("{\"totalEntities\":").Append(totalEntities).Append(",\"byProto\":{");
		bool first = true;
		foreach (KeyValuePair<string, int> kv in protoCounts) {
			if (!first) census.Append(',');
			first = false;
			census.Append('"').Append(JsonEscape(kv.Key)).Append("\":").Append(kv.Value);
		}
		census.Append("},\"machines\":{");
		first = true;
		foreach (KeyValuePair<string, Dictionary<string, int>> kv in machineStates) {
			if (!first) census.Append(',');
			first = false;
			census.Append('"').Append(JsonEscape(kv.Key)).Append("\":{\"states\":{");
			bool f2 = true;
			foreach (KeyValuePair<string, int> sv in kv.Value) {
				if (!f2) census.Append(',');
				f2 = false;
				census.Append('"').Append(JsonEscape(sv.Key)).Append("\":").Append(sv.Value);
			}
			census.Append("},\"recipes\":[");
			f2 = true;
			if (machineRecipes.TryGetValue(kv.Key, out HashSet<string> rl)) {
				foreach (string r in rl) {
					if (!f2) census.Append(',');
					f2 = false;
					census.Append('"').Append(JsonEscape(r)).Append('"');
				}
			}
			census.Append("]}");
		}
		census.Append("},\"transports\":{\"total\":").Append(transports)
			.Append(",\"fullyConnected\":").Append(connectedTransports)
			.Append("},\"priorities\":{").Append(string.Join(",", priorityRows)).Append("}}");

		topo.Append("{\"entities\":[").Append(string.Join(",", entityRows))
			.Append("],\"transports\":[").Append(string.Join(",", transportRows))
			.Append("],\"edges\":[").Append(string.Join(",", edgeRows)).Append("]}");

		stor.Append("{\"storages\":[").Append(string.Join(",", storageRows)).Append("]}");

		mach.Append("{\"machines\":[").Append(string.Join(",", machineRows))
			.Append("],\"skippedRows\":").Append(machineRowsSkipped).Append('}');

		m_censusJson = census.ToString();
		m_topologyJson = topo.ToString();
		m_storagesJson = stor.ToString();
		m_machinesJson = mach.ToString();
	}

	private static string BuildMachineRow(int id, string proto, Machine m) {
		Tile3i ctr = ((LayoutEntity)m).CenterTile;
		var sb = new StringBuilder();
		sb.Append("{\"id\":").Append(id)
			.Append(",\"proto\":\"").Append(JsonEscape(proto))
			.Append("\",\"pos\":[").Append(ctr.X).Append(',').Append(ctr.Y).Append(',').Append(ctr.Z)
			.Append("],\"state\":\"").Append(JsonEscape(m.CurrentState.ToString()))
			.Append("\",\"recipes\":[");
		bool first = true;
		foreach (var rp in m.RecipesAssigned) {
			if (!first) sb.Append(',');
			first = false;
			sb.Append('"').Append(JsonEscape(rp.Id.Value)).Append('"');
		}
		sb.Append("],\"isPaused\":").Append(m.IsPaused ? "true" : "false")
			.Append(",\"isEnabled\":").Append(m.IsEnabled ? "true" : "false")
			.Append(",\"canBePaused\":").Append(m.CanBePaused ? "true" : "false")
			.Append('}');
		return sb.ToString();
	}

	private static string BuildStorageRow(int id, string proto, string product, StorageBase st) {
		var sb = new StringBuilder();
		sb.Append("{\"id\":").Append(id)
			.Append(",\"proto\":\"").Append(JsonEscape(proto))
			.Append("\",\"product\":\"").Append(JsonEscape(product))
			.Append("\",\"quantity\":").Append(st.CurrentQuantity.Value)
			.Append(",\"capacity\":").Append(st.Capacity.Value)
			.Append(",\"percentFullRaw\":").Append(st.PercentFull.RawValue)
			.Append(",\"logisticsInDisabled\":").Append(st.IsLogisticsInputDisabled ? "true" : "false")
			.Append(",\"logisticsOutDisabled\":").Append(st.IsLogisticsOutputDisabled ? "true" : "false")
			// General priority lives on LayoutEntity (StorageBase's base);
			// every storage row carries it (recon round 2).
			.Append(",\"generalPriority\":").Append(st.GeneralPriority);
		// Slider steps + custom priorities live only on the concrete
		// Storage class (StorageBase has neither). Steps are not stored
		// as integers in the game: each step is exactly 10% of capacity,
		// so step = Percent.RawValue / 10000 (Percent.Hundred = 100000).
		if (st is Storage s) {
			sb.Append(",\"importPriority\":").Append(s.ImportPriority)
				.Append(",\"exportPriority\":").Append(s.ExportPriority)
				.Append(",\"importStep\":").Append(s.ImportUntilPercent.RawValue / 10000)
				.Append(",\"exportStep\":").Append(s.ExportFromPercent.RawValue / 10000)
				.Append(",\"transportFrom\":").Append(s.TransportFromPercent.RawValue / 10000)
				.Append(",\"transportUntil\":").Append(s.TransportUntilPercent.RawValue / 10000)
				.Append(",\"importUntilPercentRaw\":").Append(s.ImportUntilPercent.RawValue)
				.Append(",\"exportFromPercentRaw\":").Append(s.ExportFromPercent.RawValue)
				.Append(",\"transportFromPercentRaw\":").Append(s.TransportFromPercent.RawValue)
				.Append(",\"transportUntilPercentRaw\":").Append(s.TransportUntilPercent.RawValue);
		}
		sb.Append('}');
		return sb.ToString();
	}

	private static string BuildTransportRow(int id, string proto, Tile3i a, Tile3i b, Transport t) {
		var sb = new StringBuilder();
		sb.Append("{\"id\":").Append(id)
			.Append(",\"proto\":\"").Append(JsonEscape(proto))
			.Append("\",\"from\":[").Append(a.X).Append(',').Append(a.Y).Append(',').Append(a.Z)
			.Append("],\"to\":[").Append(b.X).Append(',').Append(b.Y).Append(',').Append(b.Z)
			.Append("],\"fullyConnected\":").Append(t.IsFullyConnected ? "true" : "false")
			.Append(",\"isMoving\":").Append(t.IsMoving ? "true" : "false")
			.Append('}');
		return sb.ToString();
	}

	// Handlers never throw into the sim: any failure becomes a JSON 500
	// with the route + error type, and the detail goes to the game log.
	internal static WebRouteResponse Guard(string route, Func<string> body) {
		try {
			return WebRouteResponse.Json(body());
		} catch (Exception ex) {
			Log.Error("CoiBridge: " + route + " failed: " + ex);
			return WebRouteResponse.Json(
				"{\"error\":\"bridgeError\",\"route\":\"" + route +
				"\",\"detail\":\"" + JsonEscape(ex.GetType().Name) + "\"}", 500);
		}
	}

	internal static string JsonEscape(string value) {
		if (string.IsNullOrEmpty(value)) return string.Empty;
		var sb = new StringBuilder(value.Length);
		foreach (char c in value) {
			switch (c) {
				case '\"': sb.Append("\\\""); break;
				case '\\': sb.Append("\\\\"); break;
				case '\n': sb.Append("\\n"); break;
				case '\r': sb.Append("\\r"); break;
				case '\t': sb.Append("\\t"); break;
				default:
					if (c < ' ') sb.Append("\\u").Append(((int)c).ToString("x4"));
					else sb.Append(c);
					break;
			}
		}
		return sb.ToString();
	}
}
