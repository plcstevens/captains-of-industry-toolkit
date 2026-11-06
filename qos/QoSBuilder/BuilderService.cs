using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using Mafi;
using Mafi.Collections.ImmutableCollections;
using Mafi.Core;
using Mafi.Core.Entities;
using Mafi.Core.Entities.Blueprints;
using Mafi.Core.Entities.Static.Commands;
using Mafi.Core.Factory.Transports;
using Mafi.Core.Input;
using Mafi.Core.Simulation;
using Mafi.Logging;
using Mafi.Serialization;
using QoSCore;

namespace QoSBuilder;

/// <summary>
/// The placement executor (recon 10 §Q5 design, MODDING-NOTES
/// round 12 probe). Once per in-game day (at day end) it scans the
/// jobs folder —
///
///   %APPDATA%\Captain of Industry\QoS\jobs\
///
/// — for job files written by the CoI Companion website. A job file
/// exists only because a human confirmed that exact placement on
/// the website (§12 ceremony; the file IS the confirmed order).
/// For each job the service validates, in order:
///
///   schema version · SHA-256 of the canonical payload (tamper
///   check) · not expired · gameId equals the running game's id
///   (IGameIdProvider — the same value /api/v1/game-info serves,
///   so a job confirmed against one save can never fire on
///   another) · rotation supported · blueprint string parses via
///   the game's own BlueprintsLibrary.TryParseFromString · item
///   count within the job's own cap AND the mod's config cap ·
///   every item transformable (trajectory rebuilds included).
///
/// A valid job is then CLAIMED (file renamed, jobId recorded in
/// processed.txt BEFORE anything is scheduled — single-use even
/// across a crash/restart) and executed as ONE shipped
/// BatchCreateStaticEntitiesCmd through IInputScheduler, with the
/// items copied and re-anchored exactly the way the game's own
/// blueprint placer composes them (blueprint-local coords rotated
/// about the origin, then offset by the anchor; OriginalEntityId
/// cleared; auto-built-by-parent items skipped; applyConfiguration
/// honoured). Every outcome — claimed, scheduled, or refused with
/// a named reason — is recorded to the QoS activity feed; refused
/// jobs are consumed too (renamed .failed), never left to fire
/// later. The batch command's own result is bool + last-error
/// only, so this service attests "scheduled", never "placed" —
/// the honest placement verdict is the companion's census diff.
///
/// Master switch `enabled` ships FALSE: while off, the service
/// does not even scan — job files simply wait and expire.
/// </summary>
// [ManuallyWrittenSerialization] + the hand-written Serialize /
// Deserialize pair below are REQUIRED for save safety — see the
// serialization block comment after the fields. Do not remove.
[ManuallyWrittenSerialization]
[GlobalDependency(RegistrationMode.AsEverything)]
public sealed class BuilderService : IDisposable {

	private readonly QoSBuilderMod m_mod;
	private readonly IInputScheduler m_scheduler;
	private readonly ICalendar m_calendar;
	private readonly BlueprintsLibrary m_library;
	private readonly IGameIdProvider m_gameIds;

	// ---- Save/load serialization (hand-written, game-sanctioned) ----
	//
	// WHY THIS EXISTS: the calendar's NewDayEnd event is a SAVED event
	// (Mafi.EventBase). Subscribing with Add(this, ...) stores a
	// CallbackSaveData whose Serialize() generic-writes THE OWNER —
	// this instance — into every save file. A concrete class is
	// generic-serializer-eligible only if it exposes public static
	// Serialize(T, BlobWriter) / Deserialize(BlobReader) methods
	// (MODDING-NOTES round 11). Without the pair, saving throws
	// "Failed to create generic serializer for 'BuilderService'" and
	// THE GAME CANNOT SAVE AT ALL — the suite 0.5.0 bug, fixed suite-
	// wide in 0.5.1 with exactly this pattern. The pair mirrors the
	// generator's output shape: the only persistent state written is
	// the calendar reference; every other dependency is re-resolved
	// from the game's DependencyResolver at load-finalize via
	// RegisterResolvedMember and is never serialized. This service
	// holds no other state: config is re-read from the mod instance
	// every pass, and the consumed-job record is a FILE in the jobs
	// folder (processed.txt), re-read every pass — never memory.
	private static readonly Action<object, BlobWriter> s_serializeDataDelayedAction =
		(obj, writer) => ((BuilderService)obj).SerializeData(writer);

	private static readonly Action<object, BlobReader> s_deserializeDataDelayedAction =
		(obj, reader) => ((BuilderService)obj).DeserializeData(reader);

	public static void Serialize(BuilderService value, BlobWriter writer) {
		if (writer.TryStartClassSerialization(value)) {
			writer.EnqueueDataSerialization(value, s_serializeDataDelayedAction);
		}
	}

	public static BuilderService Deserialize(BlobReader reader) {
		BuilderService result;
		if (reader.TryStartClassDeserialization<BuilderService>(out result)) {
			reader.EnqueueDataDeserialization(result, s_deserializeDataDelayedAction);
		}
		return result;
	}

	private void SerializeData(BlobWriter writer) {
		writer.WriteGeneric(m_calendar);
	}

	private void DeserializeData(BlobReader reader) {
		reader.SetField(this, "m_calendar", reader.ReadGenericAs<ICalendar>());
		reader.RegisterResolvedMember(this, "m_mod", typeof(QoSBuilderMod), isField: true);
		reader.RegisterResolvedMember(this, "m_scheduler", typeof(IInputScheduler), isField: true);
		reader.RegisterResolvedMember(this, "m_library", typeof(BlueprintsLibrary), isField: true);
		reader.RegisterResolvedMember(this, "m_gameIds", typeof(IGameIdProvider), isField: true);
	}

	public BuilderService(QoSBuilderMod mod, IInputScheduler scheduler, ICalendar calendar,
			BlueprintsLibrary library, IGameIdProvider gameIds) {
		m_mod = mod;
		m_scheduler = scheduler;
		m_calendar = calendar;
		m_library = library;
		m_gameIds = gameIds;
		m_calendar.NewDayEnd.Add(this, OnNewDayEnd);
		try {
			Directory.CreateDirectory(JobsDir());
		} catch (Exception ex) {
			QoSLog.Warning("Builder", "could not create the jobs folder: " + ex.Message);
		}
		QoSLog.Info("Builder", "builder armed (daily pass at day end; jobs folder: " + JobsDir() +
			"; master switch is the 'enabled' mod setting, default off)");
	}

	public void Dispose() {
		m_calendar.NewDayEnd.Remove(this, OnNewDayEnd);
	}

	/// <summary>The jobs folder: sibling of the QoS activity feed.</summary>
	public static string JobsDir() {
		return Path.Combine(
			Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
			"Captain of Industry", "QoS", "jobs");
	}

	private static string ProcessedPath(string jobsDir) {
		return Path.Combine(jobsDir, "processed.txt");
	}

	private void OnNewDayEnd() {
		try {
			RunPass();
		} catch (Exception ex) {
			QoSLog.Error("Builder", "daily pass failed (no jobs executed): " + ex);
		}
	}

	private void RunPass() {
		if (!m_mod.JsonConfig.GetBool("enabled", false)) {
			QoSLog.Debug("Builder", "pass skipped: 'enabled' is false");
			return;
		}
		string jobsDir = JobsDir();
		Directory.CreateDirectory(jobsDir);
		bool logJobs = m_mod.JsonConfig.GetBool("log_jobs", true);
		int configCap = Math.Max(1, m_mod.JsonConfig.GetInt("max_items_per_job", 500));
		HashSet<string> processed = ReadProcessed(jobsDir);
		string[] files = Directory.GetFiles(jobsDir, "*.json");
		Array.Sort(files, StringComparer.Ordinal);
		foreach (string file in files) {
			try {
				ProcessJob(file, jobsDir, processed, configCap, logJobs);
			} catch (Exception ex) {
				// One bad job must never stop the pass or reach the sim.
				QoSLog.Error("Builder", "job file " + Path.GetFileName(file) +
					" failed unexpectedly (left in place): " + ex.Message);
			}
		}
	}

	// ------------------------------------------------------------------
	// One job: validate -> claim -> schedule (or refuse, consuming it)
	// ------------------------------------------------------------------

	private void ProcessJob(string file, string jobsDir, HashSet<string> processed,
			int configCap, bool logJobs) {
		string fileName = Path.GetFileName(file);
		string text;
		try {
			text = File.ReadAllText(file);
		} catch (Exception ex) {
			Refuse(file, jobsDir, processed, null, null, "unreadable",
				"could not read the job file: " + ex.Message, logJobs);
			return;
		}

		Dictionary<string, object> job;
		try {
			job = BuilderJson.ParseObject(text);
		} catch (Exception ex) {
			Refuse(file, jobsDir, processed, null, null, "malformed",
				"not valid job JSON: " + ex.Message, logJobs);
			return;
		}

		string jobId, createdUtc, expiresUtc, gameId, companionUser, blueprintName, blueprintString, sha;
		long schema, anchorX, anchorY, anchorZ, rotationDeg, jobCap;
		bool applyConfiguration;
		Dictionary<string, object> anchor, limits;
		if (!BuilderJson.GetString(job, "jobId", out jobId) || jobId.Length == 0 || jobId.Length > 64 ||
				!BuilderJson.GetLong(job, "schema", out schema) ||
				!BuilderJson.GetString(job, "createdUtc", out createdUtc) ||
				!BuilderJson.GetString(job, "expiresUtc", out expiresUtc) ||
				!BuilderJson.GetString(job, "gameId", out gameId) ||
				!BuilderJson.GetString(job, "companionUser", out companionUser) ||
				!BuilderJson.GetString(job, "blueprintName", out blueprintName) ||
				!BuilderJson.GetString(job, "blueprintString", out blueprintString) ||
				!BuilderJson.GetObject(job, "anchor", out anchor) ||
				!BuilderJson.GetLong(anchor, "x", out anchorX) ||
				!BuilderJson.GetLong(anchor, "y", out anchorY) ||
				!BuilderJson.GetLong(anchor, "z", out anchorZ) ||
				!BuilderJson.GetLong(job, "rotationDeg", out rotationDeg) ||
				!BuilderJson.GetBool(job, "applyConfiguration", out applyConfiguration) ||
				!BuilderJson.GetObject(job, "limits", out limits) ||
				!BuilderJson.GetLong(limits, "maxItems", out jobCap) ||
				!BuilderJson.GetString(job, "sha256", out sha)) {
			Refuse(file, jobsDir, processed, null, null, "malformed",
				"job is missing fields or has wrong-typed fields (schema v1)", logJobs);
			return;
		}
		if (Math.Abs(anchorX) > 100000 || Math.Abs(anchorY) > 100000 || Math.Abs(anchorZ) > 1000) {
			Refuse(file, jobsDir, processed, jobId, blueprintName, "malformed",
				"anchor is outside any plausible map range", logJobs);
			return;
		}

		// Single-use: a jobId already in the record was consumed by an
		// earlier pass (executed or refused). A re-delivered copy is
		// filed away without any further action.
		if (processed.Contains(jobId)) {
			TryMove(file, file + ".done");
			QoSLog.Debug("Builder", "duplicate job file for already-consumed job " + jobId +
				" filed away (" + fileName + ")");
			return;
		}

		if (schema != 1) {
			Refuse(file, jobsDir, processed, jobId, blueprintName, "unsupported-schema",
				"job schema is " + schema + ", this mod speaks schema 1", logJobs);
			return;
		}

		string canonical = BuilderJson.Canonical(jobId, schema, createdUtc, expiresUtc, gameId,
			companionUser, blueprintName, blueprintString, anchorX, anchorY, anchorZ,
			rotationDeg, applyConfiguration, jobCap);
		if (!string.Equals(BuilderJson.Sha256Hex(canonical), sha, StringComparison.OrdinalIgnoreCase)) {
			Refuse(file, jobsDir, processed, jobId, blueprintName, "bad-hash",
				"sha256 does not match the canonical payload — the file was edited after confirm", logJobs);
			return;
		}

		DateTime expires;
		if (!DateTime.TryParse(expiresUtc, CultureInfo.InvariantCulture,
				DateTimeStyles.RoundtripKind, out expires)) {
			Refuse(file, jobsDir, processed, jobId, blueprintName, "malformed",
				"expiresUtc is not a parseable timestamp", logJobs);
			return;
		}
		if (expires.ToUniversalTime() <= DateTime.UtcNow) {
			Refuse(file, jobsDir, processed, jobId, blueprintName, "expired",
				"job expired at " + expiresUtc + " (confirm again to re-issue)", logJobs);
			return;
		}

		string runningGameId = m_gameIds.GameId.ToString(CultureInfo.InvariantCulture);
		if (!string.Equals(gameId, runningGameId, StringComparison.Ordinal)) {
			Refuse(file, jobsDir, processed, jobId, blueprintName, "wrong-game",
				"job is pinned to game " + gameId + " but the running game is " + runningGameId, logJobs);
			return;
		}

		if (rotationDeg != 0 && rotationDeg != 90 && rotationDeg != 180 && rotationDeg != 270) {
			Refuse(file, jobsDir, processed, jobId, blueprintName, "unsupported-rotation",
				"rotationDeg " + rotationDeg + " is not one of 0/90/180/270", logJobs);
			return;
		}
		bool reflected;
		if (BuilderJson.GetBool(job, "reflected", out reflected) && reflected) {
			Refuse(file, jobsDir, processed, jobId, blueprintName, "reflection-unsupported",
				"job schema v1 does not place mirrored blueprints", logJobs);
			return;
		}

		string parseError;
		IBlueprintItem parsedItem;
		if (!m_library.TryParseFromString(blueprintString, out parseError, out parsedItem)) {
			Refuse(file, jobsDir, processed, jobId, blueprintName, "blueprint-unparseable",
				"the game's own parser rejected the string: " + parseError, logJobs);
			return;
		}
		IBlueprint blueprint = parsedItem as IBlueprint;
		if (blueprint == null) {
			Refuse(file, jobsDir, processed, jobId, blueprintName, "not-a-blueprint",
				"the string decodes to a folder, not a single blueprint", logJobs);
			return;
		}

		long effectiveCap = Math.Min(jobCap, configCap);
		if (blueprint.Items.Length > effectiveCap) {
			Refuse(file, jobsDir, processed, jobId, blueprintName, "over-item-cap",
				"blueprint has " + blueprint.Items.Length + " items, cap is " + effectiveCap +
				" (job cap " + jobCap + ", mod cap " + configCap + ")", logJobs);
			return;
		}

		var anchorTile = new Tile3i((int)anchorX, (int)anchorY, (int)anchorZ);
		ImmutableArray<EntityConfigData> items;
		int skippedAutoBuilt, skippedNoTransform;
		try {
			items = BuildPlacements(blueprint, anchorTile, rotationDeg,
				out skippedAutoBuilt, out skippedNoTransform);
		} catch (JobRefusalException refusal) {
			Refuse(file, jobsDir, processed, jobId, blueprintName, refusal.Reason,
				refusal.Message, logJobs);
			return;
		}
		if (items.Length == 0) {
			Refuse(file, jobsDir, processed, jobId, blueprintName, "nothing-to-place",
				"every item was skipped (auto-built by parent, or no transform/trajectory)", logJobs);
			return;
		}

		// ---- CLAIM: rename + record BEFORE scheduling, so the job is
		// single-use even if the game crashes mid-schedule. ----
		string claimedPath = file + ".claimed";
		try {
			if (File.Exists(claimedPath)) {
				File.Delete(claimedPath);
			}
			File.Move(file, claimedPath);
		} catch (Exception ex) {
			QoSLog.Error("Builder", "could not claim job " + jobId + " (" + fileName + "): " +
				ex.Message + " — job left unclaimed, will retry next pass");
			return;
		}
		AppendProcessed(jobsDir, processed, jobId);
		QoSEventLog.Record("QoSBuilder", QoSBuilderMod.Version, "placement", "JobClaimed",
			"claimed job " + jobId + " (" + blueprintName + ") from " + companionUser,
			new Dictionary<string, object> {
				{ "jobId", jobId },
				{ "blueprintName", blueprintName },
				{ "companionUser", companionUser },
				{ "gameId", gameId },
			});
		if (logJobs) {
			QoSLog.Info("Builder", "claimed job " + jobId + " (" + blueprintName + ")");
		}

		// ---- SCHEDULE the one batch command. ----
		try {
			m_scheduler.ScheduleInputCmd(new BatchCreateStaticEntitiesCmd(items,
				BuildMiniZippersMode.DeferToProto,
				allowValidationSuppression: false,
				applyConfiguration: applyConfiguration));
		} catch (Exception ex) {
			QoSEventLog.Record("QoSBuilder", QoSBuilderMod.Version, "placement", "JobScheduleFailed",
				"job " + jobId + " (" + blueprintName + ") claimed but scheduling threw: " + ex.Message,
				new Dictionary<string, object> {
					{ "jobId", jobId },
					{ "blueprintName", blueprintName },
					{ "error", ex.Message },
				});
			QoSLog.Error("Builder", "scheduling job " + jobId + " failed: " + ex.Message);
			TryMove(claimedPath, file + ".failed");
			return;
		}
		TryMove(claimedPath, file + ".done");

		var data = new Dictionary<string, object> {
			{ "jobId", jobId },
			{ "blueprintName", blueprintName },
			{ "companionUser", companionUser },
			{ "items", items.Length },
			{ "blueprintItems", blueprint.Items.Length },
			{ "skippedAutoBuilt", skippedAutoBuilt },
			{ "skippedNoTransform", skippedNoTransform },
			{ "surfacesIgnored", blueprint.Surfaces.Length },
			{ "decalsIgnored", blueprint.Decals.Length },
			{ "anchorX", (long)anchorTile.X },
			{ "anchorY", (long)anchorTile.Y },
			{ "anchorZ", (long)anchorTile.Z },
			{ "rotationDeg", rotationDeg },
			{ "applyConfiguration", applyConfiguration },
		};
		if (blueprint.ProtosThatFailedToLoad.HasValue) {
			data["protosThatFailedToLoad"] = blueprint.ProtosThatFailedToLoad.Value;
		}
		QoSEventLog.Record("QoSBuilder", QoSBuilderMod.Version, "placement", "JobScheduled",
			"scheduled placement of '" + blueprintName + "': " + items.Length + " items at (" +
			anchorTile.X + "," + anchorTile.Y + "," + anchorTile.Z + "), rotation " + rotationDeg +
			" — scheduled, not yet verified placed (companion census diff is the verdict)",
			data);
		if (logJobs) {
			QoSLog.Info("Builder", "scheduled job " + jobId + ": " + items.Length +
				" items of '" + blueprintName + "' at (" + anchorTile.X + "," + anchorTile.Y + "," +
				anchorTile.Z + ") rotation " + rotationDeg + " (skipped " + skippedAutoBuilt +
				" auto-built, " + skippedNoTransform + " transformless)");
		}
	}

	// ------------------------------------------------------------------
	// Placement composition — the game's own recipe (recon 10 §Q4),
	// restricted to translation + quarter-turn rotation about the
	// blueprint origin. Quarter-turns use the shipped rotation math
	// (Tile3i/RelTile3i.RotatedAroundZ), never hand-rolled geometry.
	// ------------------------------------------------------------------

	private sealed class JobRefusalException : Exception {
		public string Reason { get; private set; }
		public JobRefusalException(string reason, string detail) : base(detail) {
			Reason = reason;
		}
	}

	private static ImmutableArray<EntityConfigData> BuildPlacements(IBlueprint blueprint,
			Tile3i anchor, long rotationDeg, out int skippedAutoBuilt, out int skippedNoTransform) {
		Rotation90 rot = rotationDeg == 90 ? Rotation90.Deg90
			: rotationDeg == 180 ? Rotation90.Deg180
			: rotationDeg == 270 ? Rotation90.Deg270
			: Rotation90.Deg0;
		skippedAutoBuilt = 0;
		skippedNoTransform = 0;
		var placed = new List<EntityConfigData>();
		foreach (EntityConfigData item in blueprint.Items) {
			if (BlueprintRotationUtils.ShouldSkipForPlacement(item)) {
				skippedAutoBuilt++;
				continue;
			}
			EntityConfigData copy = item.CreateCopy();
			copy.OriginalEntityId = null;
			bool hasTransform = copy.Transform.HasValue;
			bool hasTrajectory = copy.Trajectory.HasValue;
			if (!hasTransform && !hasTrajectory) {
				skippedNoTransform++;
				continue;
			}
			if (rotationDeg != 0 && copy.TrackDirection.HasValue) {
				throw new JobRefusalException("track-rotation-unsupported",
					"blueprint contains train-track items with a track direction; " +
					"job schema v1 places tracks only at rotation 0");
			}
			if (hasTransform) {
				TileTransform t = copy.Transform.Value;
				Tile3i rp = t.Position.RotatedAroundZ(rot);
				var moved = new TileTransform(
					new Tile3i(rp.X + anchor.X, rp.Y + anchor.Y, rp.Z + anchor.Z),
					t.Rotation, t.IsReflected);
				copy.Transform = moved.RotateBy(rot);
			}
			if (hasTrajectory) {
				TransportTrajectory traj = copy.Trajectory.Value;
				var pivots = new Tile3i[traj.Pivots.Length];
				for (int i = 0; i < pivots.Length; i++) {
					Tile3i rp = traj.Pivots[i].RotatedAroundZ(rot);
					pivots[i] = new Tile3i(rp.X + anchor.X, rp.Y + anchor.Y, rp.Z + anchor.Z);
				}
				RelTile3i startDir = traj.StartDirection.RotatedAroundZ(rot);
				RelTile3i endDir = traj.EndDirection.RotatedAroundZ(rot);
				TransportTrajectory rebuilt;
				string error;
				if (!TransportTrajectory.TryCreateFromPivots(traj.TransportProto,
						ImmutableArray.Create(pivots), startDir, endDir,
						out rebuilt, out error,
						ignoreSoftConstraints: true,
						allowDenormalizedStartEndDirections: true,
						padStart: traj.PadStart)) {
					throw new JobRefusalException("item-transform-failed",
						"trajectory rebuild failed for " +
						(copy.Prototype.HasValue ? copy.Prototype.Value.Id.Value : "?") +
						": " + error);
				}
				copy.Trajectory = rebuilt;
				if (copy.Pillars.HasValue) {
					ImmutableArray<Tile2i> old = copy.Pillars.Value;
					var pillars = new Tile2i[old.Length];
					for (int i = 0; i < pillars.Length; i++) {
						Tile3i rp = new Tile3i(old[i].X, old[i].Y, 0).RotatedAroundZ(rot);
						pillars[i] = new Tile2i(rp.X + anchor.X, rp.Y + anchor.Y);
					}
					copy.Pillars = ImmutableArray.Create(pillars);
				}
			}
			placed.Add(copy);
		}
		return ImmutableArray.Create(placed.ToArray());
	}

	// ------------------------------------------------------------------
	// Refusals, the processed record, file moves
	// ------------------------------------------------------------------

	private void Refuse(string file, string jobsDir, HashSet<string> processed,
			string jobId, string blueprintName, string reason, string detail, bool logJobs) {
		QoSEventLog.Record("QoSBuilder", QoSBuilderMod.Version, "placement", "JobRefused",
			"refused job " + (jobId ?? Path.GetFileName(file)) + ": " + reason + " — " + detail,
			new Dictionary<string, object> {
				{ "jobId", jobId },
				{ "blueprintName", blueprintName },
				{ "reason", reason },
				{ "detail", detail },
				{ "file", Path.GetFileName(file) },
			});
		if (logJobs) {
			QoSLog.Warning("Builder", "refused job " + (jobId ?? Path.GetFileName(file)) +
				": " + reason + " — " + detail);
		}
		if (jobId != null) {
			AppendProcessed(jobsDir, processed, jobId);
		}
		TryMove(file, file + ".failed");
	}

	private static HashSet<string> ReadProcessed(string jobsDir) {
		var set = new HashSet<string>();
		try {
			string path = ProcessedPath(jobsDir);
			if (File.Exists(path)) {
				foreach (string line in File.ReadAllLines(path)) {
					string id = line.Trim();
					if (id.Length > 0) {
						set.Add(id);
					}
				}
			}
		} catch (Exception ex) {
			QoSLog.Warning("Builder", "could not read processed.txt (" + ex.Message +
				") — treating every job as unprocessed this pass");
		}
		return set;
	}

	private static void AppendProcessed(string jobsDir, HashSet<string> processed, string jobId) {
		if (!processed.Add(jobId)) {
			return;
		}
		try {
			File.AppendAllText(ProcessedPath(jobsDir), jobId + "\n");
		} catch (Exception ex) {
			// The in-memory set still protects this session; the file
			// is the cross-restart record, so say loudly that it failed.
			QoSLog.Error("Builder", "could not append job " + jobId + " to processed.txt: " +
				ex.Message + " — single-use across restarts is NOT guaranteed for this job");
		}
	}

	private static void TryMove(string from, string to) {
		try {
			if (File.Exists(to)) {
				File.Delete(to);
			}
			File.Move(from, to);
		} catch (Exception ex) {
			QoSLog.Warning("Builder", "could not rename " + Path.GetFileName(from) +
				" -> " + Path.GetFileName(to) + ": " + ex.Message);
		}
	}
}
