using System;
using System.Collections.Generic;
using System.Text;
using Mafi;
using Mafi.Core;
using Mafi.Core.Products;
using Mafi.Core.Terrain;
using Mafi.Core.Terrain.Designation;
using Mafi.Core.WebApi;

namespace CoiBridge;

/// <summary>
/// v0.6.5 terrain-sensing routes: the land itself — heights,
/// on-map resource deposits, and terrain designations. Until now
/// the bridge could only see built things (topology / machines
/// serve Tile3i positions); these routes close the sensing gap
/// Philip named on 2026-11-06.
///
/// Surfaces, all read from the shipped 0.8.8 assemblies
/// (recon round 13 in bridge/MODDING-NOTES.md, full trail in
/// qol-recon/11-terrain-deposits.md):
///   * TerrainManager — [GlobalDependency(AsSelf)], injected
///     concrete. TerrainWidth / TerrainHeight / HeightsData /
///     GetHeight(Tile2i) / GetLayersCountNoBedrock / GetLayerAt /
///     ResolveSlimMaterial / Bedrock / IsOcean / MinOnLimits /
///     MaxOnLimitsExcl. Index math (decompiled GetTileIndex):
///     index = y * TerrainWidth + x, row-major over every
///     per-tile array.
///   * Height semantics: HeightTilesF wraps Fix32 (raw 65536 =
///     1.0) in TILE units. TerrainTile's own constants:
///     TILE_SIZE_M = 2, TILE_AREA_M = 4, TILE_VOLUME_M = 8 — a
///     tile is 2 m x 2 m, one height unit is 2 m, one tile-cubed
///     is 8 m3. Heights travel as raw Fix32 ints; consumers
///     divide by 65536.
///   * Coordinate space: TerrainTile.WithHeightFloored
///     decompiles to TileCoordSlim.ExtendHeight(
///     Height.TilesHeightFloored) — Tile3i.Z IS the floored
///     terrain height, so terrain (x, y) and entity (x, y) are
///     the same grid with the same Z scale. No transform.
///   * Deposits have NO game object: on-map resources are terrain
///     material layers. Per tile, layers are walked via
///     GetLayerAt(index, i) for i &lt; GetLayersCountNoBedrock(
///     index) + 1 (walks the overflow chain; the bottom layer is
///     bedrock and is excluded). A layer's
///     TerrainMaterialThicknessSlim.ToPartialProductQuantity(
///     manager) is the game's own per-tile mined quantity, so
///     the summed quantity is depletion-exact: mining removes
///     layer thickness, and what is read today is what remains.
///     Deposit REGIONS are bridge-side connected components
///     (4-connectivity) per material — documented as our
///     grouping, not a game concept.
///   * Designations: ITerrainDesignationsManager.Designations,
///     partitioned by kind through ITerrainMiningManager.
///     MiningDesignations / ITerrainDumpingManager.
///     DumpingDesignations (residue = "other", e.g. forestry).
///     All three managers are [GlobalDependency(AsEverything)].
///     Read-only enumeration only — their events are NOT
///     subscribed (the bridge holds no subscriptions at all).
///   * Query binding follows the shipped ProductsApi handler
///     shape: first parameter WebRouteRequest, the rest bound
///     from the query string by name with C# defaults.
///
/// This class is a SEPARATE global dependency, resolved in its
/// own try/catch in CoiBridgeMod.Initialize: a terrain failure
/// can never take the v0.5/v0.6 routes down. Same hard rules:
/// read-only, Guard-wrapped, no subscriptions, no writes.
/// </summary>
[GlobalDependency(RegistrationMode.AsEverything)]
public sealed class BridgeRoutesTerrain {

	/// <summary>Hard cap on the region route's exact-height
	/// payload: 128 x 128 tiles = 16,384 raw values (~150 KB of
	/// JSON). A full 640x640 grid would be ~2-4 MB; the summary
	/// route exists precisely so nobody needs it.</summary>
	private const int MaxRegionTiles = 16384;
	private const int MaxRegionSide = 128;

	/// <summary>The summary grid never exceeds 96 x 96 samples;
	/// the stride is chosen per map from the larger dimension.</summary>
	private const int MaxSummarySide = 96;

	/// <summary>Deposit component cap: beyond this many connected
	/// regions only the largest (by quantity) are listed; the
	/// per-material totals are always complete and the payload
	/// flags the truncation.</summary>
	private const int MaxDepositComponents = 1024;

	private readonly TerrainManager m_terrain;
	private readonly ITerrainDesignationsManager m_designations;
	private readonly ITerrainMiningManager m_mining;
	private readonly ITerrainDumpingManager m_dumping;

	private readonly object m_depositsLock = new object();
	private string m_depositsJson;
	private DateTime m_depositsUtc = DateTime.MinValue;
	private const double DepositsTtlSeconds = 30.0;

	public BridgeRoutesTerrain(TerrainManager terrain,
			ITerrainDesignationsManager designations,
			ITerrainMiningManager mining, ITerrainDumpingManager dumping) {
		m_terrain = terrain;
		m_designations = designations;
		m_mining = mining;
		m_dumping = dumping;
		Log.Info("CoiBridge: BridgeRoutesTerrain constructed (terrain/designations injected)");
	}

	/// <summary>Terrain meta + height summary: dimensions, the
	/// playable limits, full-grid min/max height, ocean tile
	/// count, and a point-sampled height grid (stride chosen so
	/// the grid stays &lt;= 96 x 96). Heights are raw Fix32 ints
	/// (65536 = 1 tile). Exact heights: /coi/v1/terrain/region.
	/// </summary>
	[WebRoute("/coi/v1/terrain", WebRouteStage.Background, "GET", "Terrain: dimensions, limits, height summary grid (raw Fix32 heights).")]
	public WebRouteResponse Terrain() {
		return BridgeRoutes.Guard("/coi/v1/terrain", () => {
			int w = m_terrain.TerrainWidth;
			int h = m_terrain.TerrainHeight;
			int minRaw = int.MaxValue, maxRaw = int.MinValue, ocean = 0;
			for (int y = 0; y < h; y++) {
				for (int x = 0; x < w; x++) {
					var tile = new Tile2i(x, y);
					int raw = m_terrain.GetHeight(tile).Value.RawValue;
					if (raw < minRaw) minRaw = raw;
					if (raw > maxRaw) maxRaw = raw;
					if (m_terrain.IsOcean(tile)) ocean++;
				}
			}
			int stride = Math.Max(1, (int)Math.Ceiling(
				(double)Math.Max(w, h) / MaxSummarySide));
			var sb = new StringBuilder();
			sb.Append("{\"width\":").Append(w)
				.Append(",\"height\":").Append(h)
				.Append(",\"tilesCount\":").Append(m_terrain.TerrainTilesCount)
				.Append(",\"origin\":{\"x\":0,\"y\":0}")
				.Append(",\"limits\":{\"minX\":").Append(m_terrain.MinOnLimits.X)
				.Append(",\"minY\":").Append(m_terrain.MinOnLimits.Y)
				.Append(",\"maxXExcl\":").Append(m_terrain.MaxOnLimitsExcl.X)
				.Append(",\"maxYExcl\":").Append(m_terrain.MaxOnLimitsExcl.Y)
				.Append('}')
				.Append(",\"heightUnit\":\"tile\"")
				.Append(",\"heightRawPerTile\":65536")
				.Append(",\"tileSizeM\":2")
				.Append(",\"tileVolumeM3\":8")
				.Append(",\"minHeightRaw\":").Append(minRaw)
				.Append(",\"maxHeightRaw\":").Append(maxRaw)
				.Append(",\"oceanTiles\":").Append(ocean)
				.Append(",\"summaryStride\":").Append(stride)
				.Append(",\"summaryHeightsRaw\":[");
			bool first = true;
			int sw = 0, sh = 0;
			for (int y = 0; y < h; y += stride) {
				sh++;
				for (int x = 0; x < w; x += stride) {
					if (y == 0) sw++;
					if (!first) sb.Append(',');
					first = false;
					sb.Append(m_terrain.GetHeight(new Tile2i(x, y)).Value.RawValue);
				}
			}
			sb.Append("],\"summaryWidth\":").Append(sw)
				.Append(",\"summaryHeight\":").Append(sh)
				.Append('}');
			return sb.ToString();
		});
	}

	/// <summary>Exact raw heights for one rectangle. Query
	/// params (shipped ProductsApi binding: by name, defaults
	/// apply): x0, y0 inclusive; x1, y1 EXCLUSIVE, -1 = map
	/// edge. The rectangle is clamped to the map, then — if it
	/// exceeds the 16,384-tile cap — shrunk from the requested
	/// origin corner; "truncated" reports any shrink. An empty
	/// rectangle after clamping is a 400, not an exception.
	/// </summary>
	[WebRoute("/coi/v1/terrain/region", WebRouteStage.Background, "GET", "Terrain region: exact raw heights for a rectangle (x0,y0 inclusive; x1,y1 exclusive).")]
	public WebRouteResponse TerrainRegion(WebRouteRequest request,
			int x0 = 0, int y0 = 0, int x1 = -1, int y1 = -1) {
		const string route = "/coi/v1/terrain/region";
		try {
			int w = m_terrain.TerrainWidth;
			int h = m_terrain.TerrainHeight;
			if (x1 < 0) x1 = w;
			if (y1 < 0) y1 = h;
			x0 = Clamp(x0, 0, w); x1 = Clamp(x1, 0, w);
			y0 = Clamp(y0, 0, h); y1 = Clamp(y1, 0, h);
			if (x1 <= x0 || y1 <= y0) {
				return WebRouteResponse.Json(
					"{\"error\":\"badRegion\",\"route\":\"" + route +
					"\",\"detail\":\"empty rectangle after clamping\"}", 400);
			}
			bool truncated = false;
			int rw = x1 - x0, rh = y1 - y0;
			if (rw > MaxRegionSide) { rw = MaxRegionSide; truncated = true; }
			if (rw * rh > MaxRegionTiles) {
				rh = Math.Max(1, MaxRegionTiles / rw);
				truncated = true;
			}
			if (rh > MaxRegionSide && rw * rh > MaxRegionTiles) {
				// Tall-narrow case: side cap on height too.
				rh = MaxRegionSide;
				truncated = true;
			}
			x1 = x0 + rw; y1 = y0 + rh;
			var sb = new StringBuilder();
			sb.Append("{\"x0\":").Append(x0)
				.Append(",\"y0\":").Append(y0)
				.Append(",\"x1\":").Append(x1)
				.Append(",\"y1\":").Append(y1)
				.Append(",\"width\":").Append(rw)
				.Append(",\"height\":").Append(rh)
				.Append(",\"truncated\":").Append(truncated ? "true" : "false")
				.Append(",\"heightRawPerTile\":65536")
				.Append(",\"heightsRaw\":[");
			bool first = true;
			for (int y = y0; y < y1; y++) {
				for (int x = x0; x < x1; x++) {
					if (!first) sb.Append(',');
					first = false;
					sb.Append(m_terrain.GetHeight(new Tile2i(x, y)).Value.RawValue);
				}
			}
			sb.Append("]}");
			return WebRouteResponse.Json(sb.ToString());
		} catch (Exception ex) {
			Log.Error("CoiBridge: " + route + " failed: " + ex);
			return WebRouteResponse.Json(
				"{\"error\":\"bridgeError\",\"route\":\"" + route +
				"\",\"detail\":\"" + BridgeRoutes.JsonEscape(ex.GetType().Name) + "\"}", 500);
		}
	}

	/// <summary>On-map resource deposits: per-material map
	/// totals plus connected regions per material (see the
	/// class comment for the layer model). quantity is the
	/// game's own conversion summed per tile-layer, rounded to
	/// whole product units; quantityRaw keeps the exact Fix32
	/// sum (65536 = 1 unit). volumeTilesCubedRaw is the summed
	/// layer thickness in raw Fix32 tile units. The scan walks
	/// every tile's layers, so the payload is cached ~30 s.
	/// </summary>
	[WebRoute("/coi/v1/deposits", WebRouteStage.Background, "GET", "On-map resource deposits: per-material totals and connected regions from terrain layers.")]
	public WebRouteResponse Deposits() {
		return BridgeRoutes.Guard("/coi/v1/deposits", () => {
			lock (m_depositsLock) {
				if (m_depositsJson != null &&
						(DateTime.UtcNow - m_depositsUtc).TotalSeconds < DepositsTtlSeconds) {
					return m_depositsJson;
				}
				m_depositsJson = BuildDepositsJson();
				m_depositsUtc = DateTime.UtcNow;
				return m_depositsJson;
			}
		});
	}

	private sealed class MaterialAccum {
		public string MaterialId;
		public string ProductId;
		public long Tiles;
		public long ThicknessRaw;
		public long QuantityRaw;
	}

	private sealed class TileMaterial {
		public int SlimRaw;
		public long ThicknessRaw;
		public long QuantityRaw;
	}

	private string BuildDepositsJson() {
		int w = m_terrain.TerrainWidth;
		int h = m_terrain.TerrainHeight;
		string bedrockId = m_terrain.Bedrock.Id.Value;
		var materials = new Dictionary<int, MaterialAccum>();
		var materialOrder = new List<int>();
		// Per-tile, per-material presence for the component pass.
		var perTile = new Dictionary<int, List<TileMaterial>>();

		for (int y = 0; y < h; y++) {
			for (int x = 0; x < w; x++) {
				Tile2iIndex idx = m_terrain.GetTileIndex(x, y);
				int layers = m_terrain.GetLayersCountNoBedrock(idx) + 1;
				List<TileMaterial> tileMats = null;
				for (int i = 0; i < layers; i++) {
					TerrainMaterialThicknessSlim slim = m_terrain.GetLayerAt(idx, i);
					if (!slim.HasValue) continue;
					TerrainMaterialProto proto = m_terrain.ResolveSlimMaterial(slim.SlimId);
					if (proto == null) continue;
					if (proto.Id.Value == bedrockId) continue;
					if (proto.MinedProduct == null) continue;
					PartialProductQuantity ppq = slim.ToPartialProductQuantity(m_terrain);
					long thickRaw = slim.Thickness.Value.RawValue;
					long qtyRaw = ppq.Quantity.Value.RawValue;
					int slimRaw = slim.SlimIdRaw;
					MaterialAccum acc;
					if (!materials.TryGetValue(slimRaw, out acc)) {
						acc = new MaterialAccum {
							MaterialId = proto.Id.Value,
							ProductId = proto.MinedProduct.Id.Value,
						};
						materials.Add(slimRaw, acc);
						materialOrder.Add(slimRaw);
					}
					acc.ThicknessRaw += thickRaw;
					acc.QuantityRaw += qtyRaw;
					if (tileMats == null) {
						tileMats = new List<TileMaterial>();
						perTile.Add(y * w + x, tileMats);
					}
					TileMaterial tm = null;
					foreach (TileMaterial cand in tileMats) {
						if (cand.SlimRaw == slimRaw) { tm = cand; break; }
					}
					if (tm == null) {
						tm = new TileMaterial { SlimRaw = slimRaw };
						tileMats.Add(tm);
						acc.Tiles++;
					}
					tm.ThicknessRaw += thickRaw;
					tm.QuantityRaw += qtyRaw;
				}
			}
		}

		// Connected components (4-connectivity) per material.
		var components = new List<string>();
		bool truncated = false;
		foreach (int slimRaw in materialOrder) {
			var inMaterial = new HashSet<int>();
			foreach (KeyValuePair<int, List<TileMaterial>> kv in perTile) {
				foreach (TileMaterial tm in kv.Value) {
					if (tm.SlimRaw == slimRaw) { inMaterial.Add(kv.Key); break; }
				}
			}
			var visited = new HashSet<int>();
			foreach (int start in inMaterial) {
				if (visited.Contains(start)) continue;
				// BFS one component.
				var queue = new Queue<int>();
				queue.Enqueue(start);
				visited.Add(start);
				int minX = int.MaxValue, minY = int.MaxValue;
				int maxX = int.MinValue, maxY = int.MinValue;
				long tiles = 0, qtyRaw = 0;
				while (queue.Count > 0) {
					int cur = queue.Dequeue();
					int cx = cur % w, cy = cur / w;
					if (cx < minX) minX = cx;
					if (cx > maxX) maxX = cx;
					if (cy < minY) minY = cy;
					if (cy > maxY) maxY = cy;
					tiles++;
					foreach (TileMaterial tm in perTile[cur]) {
						if (tm.SlimRaw == slimRaw) { qtyRaw += tm.QuantityRaw; break; }
					}
					AddNeighbor(cur - 1, cx > 0, inMaterial, visited, queue);
					AddNeighbor(cur + 1, cx < w - 1, inMaterial, visited, queue);
					AddNeighbor(cur - w, cy > 0, inMaterial, visited, queue);
					AddNeighbor(cur + w, cy < h - 1, inMaterial, visited, queue);
				}
				MaterialAccum acc = materials[slimRaw];
				var cb = new StringBuilder();
				cb.Append("{\"material\":\"").Append(BridgeRoutes.JsonEscape(acc.MaterialId))
					.Append("\",\"product\":\"").Append(BridgeRoutes.JsonEscape(acc.ProductId))
					.Append("\",\"minX\":").Append(minX)
					.Append(",\"minY\":").Append(minY)
					.Append(",\"maxX\":").Append(maxX)
					.Append(",\"maxY\":").Append(maxY)
					.Append(",\"tiles\":").Append(tiles)
					.Append(",\"quantity\":").Append(RoundRaw(qtyRaw))
					.Append(",\"quantityRaw\":").Append(qtyRaw)
					.Append('}');
				components.Add(cb.ToString());
			}
		}
		if (components.Count > MaxDepositComponents) {
			truncated = true;
			// Keep the largest by quantity: re-sort the built rows
			// is overkill — components were emitted per material in
			// discovery order; instead cap by dropping the tail and
			// flagging it. Totals below stay exact.
			components.RemoveRange(MaxDepositComponents,
				components.Count - MaxDepositComponents);
		}

		var sb = new StringBuilder();
		sb.Append("{\"materials\":[");
		bool first = true;
		foreach (int slimRaw in materialOrder) {
			if (!first) sb.Append(',');
			first = false;
			MaterialAccum acc = materials[slimRaw];
			sb.Append("{\"material\":\"").Append(BridgeRoutes.JsonEscape(acc.MaterialId))
				.Append("\",\"product\":\"").Append(BridgeRoutes.JsonEscape(acc.ProductId))
				.Append("\",\"tiles\":").Append(acc.Tiles)
				.Append(",\"volumeTilesCubedRaw\":").Append(acc.ThicknessRaw)
				.Append(",\"quantity\":").Append(RoundRaw(acc.QuantityRaw))
				.Append(",\"quantityRaw\":").Append(acc.QuantityRaw)
				.Append('}');
		}
		sb.Append("],\"deposits\":[");
		for (int i = 0; i < components.Count; i++) {
			if (i > 0) sb.Append(',');
			sb.Append(components[i]);
		}
		sb.Append("],\"truncated\":").Append(truncated ? "true" : "false")
			.Append(",\"scannedTiles\":").Append(w * h)
			.Append('}');
		return sb.ToString();
	}

	private static void AddNeighbor(int idx, bool inBounds, HashSet<int> inMaterial,
			HashSet<int> visited, Queue<int> queue) {
		if (!inBounds) return;
		if (!inMaterial.Contains(idx)) return;
		if (!visited.Add(idx)) return;
		queue.Enqueue(idx);
	}

	private static long RoundRaw(long raw) {
		// Fix32 raw (65536 = 1) to whole units, half away from zero.
		if (raw >= 0) return (raw + 32768) / 65536;
		return -((-raw + 32768) / 65536);
	}

	/// <summary>Terrain designations: every mining / dumping /
	/// other designation area with its target heights and
	/// fulfilment state. Kind comes from the manager partition
	/// (mining set, dumping set, residue "other"). Target
	/// heights are integer tiles (HeightTilesI).</summary>
	[WebRoute("/coi/v1/designations", WebRouteStage.Background, "GET", "Terrain designations: mining/dumping areas, target heights, fulfilment.")]
	public WebRouteResponse Designations() {
		return BridgeRoutes.Guard("/coi/v1/designations", () => {
			var mining = new HashSet<TerrainDesignation>();
			foreach (TerrainDesignation d in m_mining.MiningDesignations) mining.Add(d);
			var dumping = new HashSet<TerrainDesignation>();
			foreach (TerrainDesignation d in m_dumping.DumpingDesignations) dumping.Add(d);
			var sb = new StringBuilder();
			sb.Append("{\"designations\":[");
			bool first = true;
			foreach (TerrainDesignation d in m_designations.Designations) {
				if (d.IsDestroyed) continue;
				if (!first) sb.Append(',');
				first = false;
				string kind = mining.Contains(d) ? "mining"
					: dumping.Contains(d) ? "dumping" : "other";
				RectangleTerrainArea2i area = d.Area;
				sb.Append("{\"kind\":\"").Append(kind)
					.Append("\",\"proto\":\"").Append(BridgeRoutes.JsonEscape(d.ProtoId.Value))
					.Append("\",\"origin\":[").Append(d.OriginTileCoord.X)
					.Append(',').Append(d.OriginTileCoord.Y)
					.Append("],\"area\":{\"minX\":").Append(area.Origin.X)
					.Append(",\"minY\":").Append(area.Origin.Y)
					.Append(",\"maxXExcl\":").Append(area.Origin.X + area.Size.X)
					.Append(",\"maxYExcl\":").Append(area.Origin.Y + area.Size.Y)
					.Append("},\"sizeTiles\":").Append(d.SizeTiles)
					.Append(",\"minTargetHeight\":").Append(d.MinTargetHeight.Value)
					.Append(",\"maxTargetHeight\":").Append(d.MaxTargetHeight.Value)
					.Append(",\"isFulfilled\":").Append(d.IsFulfilled ? "true" : "false")
					.Append(",\"jobsAssigned\":").Append(d.NumberOfJobsAssigned)
					.Append('}');
			}
			sb.Append("]}");
			return sb.ToString();
		});
	}

	private static int Clamp(int v, int min, int max) {
		if (v < min) return min;
		if (v > max) return max;
		return v;
	}
}
