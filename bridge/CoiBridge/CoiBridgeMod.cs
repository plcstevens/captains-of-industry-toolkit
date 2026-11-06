using System;
using Mafi;
using Mafi.Base;
using Mafi.Collections;
using Mafi.Core;
using Mafi.Core.Mods;
using Mafi.Core.Prototypes;
using Mafi.Core.WebApi;

namespace CoiBridge;

/// <summary>
/// CoiBridge mod entry point.
///
/// The shape of this class is copied from shipped, working mods and verified
/// against the game's own assemblies on 2026-11-02 (sources:
/// bridge/MODDING-NOTES.md). It extends the template's
/// <see cref="DataOnlyMod"/> and additionally implements <see cref="IMod"/>
/// explicitly — the pattern used by the BalancerPlanner mod — because
/// DataOnlyMod's RegisterDependencies/Initialize are non-virtual no-ops.
///
/// Route registration mechanics (verified from Mafi.Core.dll): the built-in
/// WebApiServer, on first start, scans every instance in the resolver's
/// AllResolvedInstances for [WebRoute] methods. Initialize below resolves
/// <see cref="BridgeRoutes"/>, which is what puts it in that set. If the web
/// server is somehow already running when the mod initializes (late enable),
/// Initialize also calls WebApiServer.ScanObjectForRoutes directly.
///
/// The mod registers NO prototypes (products, machines, recipes, research):
/// a Tier 1 read-only bridge must leave the game content-identical to
/// vanilla, so removing the mod folder fully removes the mod (SPEC §1/§3).
/// </summary>
public sealed class CoiBridgeMod : DataOnlyMod, IMod {

	// Mod constructor will be called on mod loading.
	public CoiBridgeMod(ModManifest manifest) : base(manifest) {
		// Log output goes to the log file and can also be shown in the in-game
		// console with the `also_log_to_console` command.
		Log.Info("CoiBridge: constructed");
	}

	/// <summary>
	/// Registers game content. CoiBridge registers nothing on purpose: no
	/// protos means no content is added to saves, and the mod stays removable
	/// at any time (SPEC section 1, Tier 1 safety posture). ExampleMod calls
	/// registrator.RegisterAllProducts() / RegisterData&lt;...&gt;() here; we do not.
	/// </summary>
	public override void RegisterPrototypes(ProtoRegistrator registrator) {
		Log.Info("CoiBridge: registering prototypes (none — read-only bridge by design)");
	}

	bool IMod.IsUiOnly { get { return false; } }

	/// <summary>
	/// Registers the bridge's global dependency (<see cref="BridgeRoutes"/>)
	/// with the game's dependency-injection builder, so the resolver can
	/// construct it and its [WebRoute] methods are picked up by the built-in
	/// web server (Update 4.3). BridgeRoutes also carries the
	/// [GlobalDependency] attribute — the shipped PriorityManager mod
	/// registers its controllers both ways.
	/// </summary>
	void IMod.RegisterDependencies(DependencyResolverBuilder depBuilder, ProtosDb protosDb, bool gameWasLoaded) {
		depBuilder.RegisterDependency<BridgeRoutes>().AsSelf();
		depBuilder.RegisterDependency<BridgeRoutesV6>().AsSelf();
		depBuilder.RegisterDependency<BridgeRoutesTerrain>().AsSelf();
		Log.Info("CoiBridge: BridgeRoutes + BridgeRoutesV6 + BridgeRoutesTerrain registered with the dependency builder");
	}

	/// <summary>
	/// Runs once the game is initialized (gameWasLoaded distinguishes a
	/// loaded save from a new game; the bridge behaves the same for both).
	/// Hands the live resolver to the discovery report — metadata only, the
	/// report never invokes it — and resolves <see cref="BridgeRoutes"/> so
	/// the routes dependency is constructed and present in the resolver's
	/// resolved-instances set before the web server first scans it.
	/// </summary>
	void IMod.Initialize(DependencyResolver resolver, bool gameWasLoaded) {
		DiscoveryReport.CaptureResolver(resolver);
		try {
			BridgeRoutes routes = resolver.Resolve<BridgeRoutes>();
			Log.Info("CoiBridge: BridgeRoutes resolved at Initialize");
			// Late-join case: the web server is already running (its one-time
			// scan of resolved instances already happened), so register our
			// routes explicitly. In the normal order (mod loads first,
			// `web_api_start` later) the server's own scan finds the resolved
			// BridgeRoutes instance and this branch does nothing.
			WebApiServer server = resolver.Resolve<WebApiServer>();
			if (server.IsRunning) {
				int added = server.ScanObjectForRoutes(routes);
				Log.Info("CoiBridge: web server already running; scanned routes, added " + added);
			}
		} catch (Exception ex) {
			Log.Error("CoiBridge: route setup at Initialize failed: " + ex.Message);
		}
		// v0.6 routes resolve in their OWN try/catch: if one of the new
		// managers cannot be injected, only the v0.6 routes are lost —
		// the v0.5 routes above keep working.
		try {
			BridgeRoutesV6 routes6 = resolver.Resolve<BridgeRoutesV6>();
			Log.Info("CoiBridge: BridgeRoutesV6 resolved at Initialize");
			WebApiServer server6 = resolver.Resolve<WebApiServer>();
			if (server6.IsRunning) {
				int added6 = server6.ScanObjectForRoutes(routes6);
				Log.Info("CoiBridge: web server already running; scanned v0.6 routes, added " + added6);
			}
		} catch (Exception ex) {
			Log.Error("CoiBridge: v0.6 route setup failed (v0.5 routes unaffected): " + ex.Message);
		}
		// v0.6.5 terrain routes resolve in their OWN try/catch too:
		// if TerrainManager or a designation manager cannot be
		// injected, only the terrain routes are lost.
		try {
			BridgeRoutesTerrain routesT = resolver.Resolve<BridgeRoutesTerrain>();
			Log.Info("CoiBridge: BridgeRoutesTerrain resolved at Initialize");
			WebApiServer serverT = resolver.Resolve<WebApiServer>();
			if (serverT.IsRunning) {
				int addedT = serverT.ScanObjectForRoutes(routesT);
				Log.Info("CoiBridge: web server already running; scanned terrain routes, added " + addedT);
			}
		} catch (Exception ex) {
			Log.Error("CoiBridge: terrain route setup failed (other routes unaffected): " + ex.Message);
		}
	}

	void IDisposable.Dispose() {
		// Nothing to release: no files, sockets, or subscriptions of our own.
	}

	public override void MigrateJsonConfig(VersionSlim savedVersion, Dict<string, object> savedValues) {
		// CoiBridge ships no config.json and persists nothing; nothing to migrate.
	}
}
