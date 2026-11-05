using System;
using Mafi;
using Mafi.Collections;
using Mafi.Core.Game;
using Mafi.Core.GameLoop;
using Mafi.Core.Mods;
using Mafi.Core.Prototypes;
using Mafi.Unity.Ui.World;
using QoSCore;

namespace QoSWorldMap;

/// <summary>
/// QoSWorldMap — a navigation-only index of world-map mines and oil
/// rigs (QOL-MOD-PREP.md section 2). One window over
/// WorldMapManager.Mines; clicking a row centres the world map on
/// the location and opens its inspector. The mod issues NO game
/// commands, schedules NO input, and writes NO state: it is a pure
/// view, so it cannot alter a save and is safe to add/remove freely.
///
/// Depends on QoSCore (manifest: "QoSCore >= 0.1.0") for suite
/// logging + the suite registry; the QoSCore.dll reference is
/// provided by the QoSCore mod folder at runtime and is NOT copied
/// into this mod's folder. Registers NO prototypes.
///
/// UI construction is deferred to renderer-init (the
/// CoI_PriorityManager pattern): resolving UI services inside
/// Initialize itself crashed another community mod during assembly.
/// </summary>
public sealed class QoSWorldMapMod : IMod {

	/// <summary>Mod version. Keep in step with manifest.json (manifest is the truth).</summary>
	public const string Version = "0.1.0";

	public QoSWorldMapMod(ModManifest manifest) {
		Manifest = manifest;
		JsonConfig = new ModJsonConfig(this);
	}

	public ModManifest Manifest { get; }

	public bool IsUiOnly { get { return false; } }

	public Option<IConfig> ModConfig { get { return Option<IConfig>.None; } }

	public ModJsonConfig JsonConfig { get; }

	/// <summary>
	/// Navigation seam (prep section 2, live-check seam 2): created
	/// at renderer-init, before the toolbar controller is resolved,
	/// so the window's row clicks always find it in place. Null only
	/// if renderer-init never ran — in which case no window exists
	/// either, so nothing can be clicked.
	/// </summary>
	public MineNavigator Navigator { get; private set; }

	public void RegisterPrototypes(ProtoRegistrator registrator) {
		// Nothing: no content, by design (removal safety).
	}

	public void RegisterDependencies(DependencyResolverBuilder depBuilder, ProtosDb protosDb, bool gameWasLoaded) {
		depBuilder.RegisterDependency<WorldMinesWindow>().AsSelf();
		depBuilder.RegisterDependency<WorldMinesController>().AsSelf();
	}

	public void EarlyInit(DependencyResolver resolver) { }

	public void Initialize(DependencyResolver resolver, bool gameWasLoaded) {
		try {
			SuiteRegistry registry;
			if (resolver.TryResolve<SuiteRegistry>(out registry)) {
				registry.Announce("QoSWorldMap", Version);
			} else {
				QoSLog.Warning("WorldMap", "SuiteRegistry not resolvable — QoSCore loaded?");
			}
		} catch (Exception ex) {
			QoSLog.Error("WorldMap", "suite announce failed: " + ex.Message);
		}
		// PriorityManager pattern: defer ALL UI work to renderer
		// init. Inside the callback, build the navigator first
		// (resolving the game's world-map controller is live-check
		// seam 2 — a failure degrades navigation to inspector-only,
		// it must not take the window down), then resolve our own
		// controller, which registers the toolbar button.
		try {
			resolver.Resolve<IGameLoopEvents>().RegisterRendererInitState(this, () => {
				WorldMapWindow.Controller mapController = null;
				try {
					WorldMapWindow.Controller resolved;
					if (resolver.TryResolve<WorldMapWindow.Controller>(out resolved)) {
						mapController = resolved;
					}
				} catch (Exception ex) {
					QoSLog.Warning("WorldMap", "WorldMapWindow.Controller resolution threw: " + ex.Message);
				}
				Navigator = new MineNavigator(this, mapController);
				if (mapController == null) {
					QoSLog.Warning("WorldMap",
						"WorldMapWindow.Controller not resolvable — row clicks will open the inspector only, without centring the world map (live-check seam 2).");
				}
				try {
					resolver.Resolve<WorldMinesController>();
					QoSLog.Info("WorldMap", "QoSWorldMap initialised");
				} catch (Exception ex) {
					QoSLog.Error("WorldMap", "WorldMinesController failed to initialise: " + ex.Message);
				}
			});
		} catch (Exception ex) {
			QoSLog.Error("WorldMap", "renderer-init registration failed: " + ex.Message);
		}
	}

	public void MigrateJsonConfig(VersionSlim savedVersion, Dict<string, object> savedValues) {
		// Config schema v1: nothing to migrate.
	}

	public void Dispose() { }
}
