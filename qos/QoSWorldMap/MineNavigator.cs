using Mafi.Core.World.Entities;
using Mafi.Unity.Ui;
using Mafi.Unity.Ui.World;
using QoSCore;

namespace QoSWorldMap;

/// <summary>
/// The navigation seam: everything a row click does, in one place.
/// NAVIGATION ONLY — this class reads locations and asks the game's
/// own UI controllers to look at them. It issues no commands and
/// mutates no game state.
///
/// The prep's four live-check unknowns (QOL-MOD-PREP section 2) are
/// isolated here and in WorldMinesController, so the first live
/// session can settle each one with a small, local change:
///
///   Seam 1 — inspector placement. Navigate() opens the world map
///   centred on the mine, then calls the public
///   InspectorsManager.TryActivateFor(mine). What this does NOT
///   reproduce from the game's WorldMapWindow.SelectLocation is its
///   internal bookkeeping (selected-location pin highlight and the
///   near-pin inspector frame offset — cosmetic, per recon). IF the
///   live check shows the inspector landing somewhere wrong, the
///   fallback is a probe-guarded reflection replication of
///   SelectLocation's bookkeeping, and it belongs in THIS method.
///
///   Seam 2 — controller resolution. The WorldMapWindow.Controller
///   instance is resolved once at renderer-init (see
///   QoSWorldMapMod.Initialize) and handed to this class. If it
///   could not be resolved, navigation degrades to inspector-only
///   and says so in the log — the window still works as an index.
///
///   Seam 3 — fullscreen stacking. The world map window is
///   immersive-fullscreen; whether a plain ControllerConfig.Window
///   mod window and the inspector stack sanely against it is a live
///   check. The mitigation is built in: the config flag
///   'navigate_opens_map' (default true) flips Navigate() to
///   inspector-only if opening the map misbehaves.
///
///   Seam 4 — toolbar slot: see WorldMinesController.ToolbarOrder.
/// </summary>
public sealed class MineNavigator {

	private readonly QoSWorldMapMod m_mod;
	private readonly WorldMapWindow.Controller m_mapController;
	private bool m_warnedNoController;

	public MineNavigator(QoSWorldMapMod mod, WorldMapWindow.Controller mapController) {
		m_mod = mod;
		m_mapController = mapController;
	}

	/// <summary>Centre the world map on the mine and open its inspector.</summary>
	public void Navigate(WorldMapMine mine, InspectorsManager inspectors) {
		// Config is re-read on every navigation (suite convention:
		// no caching), so seam 3's mitigation takes effect without
		// a restart.
		bool openMap = m_mod.JsonConfig.GetBool("navigate_opens_map", true);
		if (openMap) {
			if (m_mapController != null) {
				// Public API; ActivateSelf()s the map window if it
				// is closed, then centres on the location.
				m_mapController.OpenAndCenterOnLocation(mine.Location);
			} else if (!m_warnedNoController) {
				m_warnedNoController = true;
				QoSLog.Warning("WorldMap",
					"row click: no world-map controller (seam 2) — opening the inspector without centring the map.");
			}
		}
		inspectors.TryActivateFor(mine);
	}
}
