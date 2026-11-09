using System;
using Mafi;
using Mafi.Collections;
using Mafi.Core.Game;
using Mafi.Core.Mods;
using Mafi.Core.Prototypes;
using QoSCore;

namespace QoSChickens;

/// <summary>
/// QoSChickens — first feature mod of the QoS suite. A policy layer
/// over the game's own animal-move command (see ChickenBalancer and
/// QOL-MOD-PREP.md section 1 for the design it implements).
///
/// Depends on QoSCore (manifest: "QoSCore >= 0.1.0") for suite
/// logging + the suite registry; the QoSCore.dll reference is
/// provided by the QoSCore mod folder at runtime and is NOT copied
/// into this mod's folder. Registers NO prototypes — the mod is
/// pure behaviour over shipped game systems and registers no
/// content. NOTE (0.1.2): the balancer's calendar subscription
/// IS serialized into saves written while the mod is installed
/// (the sanctioned serializer pair on ChickenBalancer), so the
/// manifest sets can_remove_from_saved_game=false — the game
/// blocks removing the mod from such a save rather than letting
/// the save become unloadable.
/// </summary>
public sealed class QoSChickensMod : IMod {

	/// <summary>Mod version. Keep in step with manifest.json (manifest is the truth).</summary>
	public const string Version = "0.1.2";

	public QoSChickensMod(ModManifest manifest) {
		Manifest = manifest;
		JsonConfig = new ModJsonConfig(this);
	}

	public ModManifest Manifest { get; }

	public bool IsUiOnly { get { return false; } }

	public Option<IConfig> ModConfig { get { return Option<IConfig>.None; } }

	public ModJsonConfig JsonConfig { get; }

	public void RegisterPrototypes(ProtoRegistrator registrator) {
		// Nothing: no content, by design (removal safety).
	}

	public void RegisterDependencies(DependencyResolverBuilder depBuilder, ProtosDb protosDb, bool gameWasLoaded) {
		depBuilder.RegisterDependency<ChickenBalancer>().AsSelf();
	}

	public void EarlyInit(DependencyResolver resolver) { }

	public void Initialize(DependencyResolver resolver, bool gameWasLoaded) {
		// Announce to the suite registry (QoSCore). The manifest
		// dependency guarantees QoSCore loaded first; if the
		// registry still cannot resolve, log it and carry on — the
		// balancer does not depend on the registry to function.
		try {
			SuiteRegistry registry;
			if (resolver.TryResolve<SuiteRegistry>(out registry)) {
				registry.Announce("QoSChickens", Version);
			} else {
				QoSLog.Warning("Chickens", "SuiteRegistry not resolvable — QoSCore loaded?");
			}
		} catch (Exception ex) {
			QoSLog.Error("Chickens", "suite announce failed: " + ex.Message);
		}
		try {
			resolver.Resolve<ChickenBalancer>();
			QoSLog.Info("Chickens", "QoSChickens initialised");
		} catch (Exception ex) {
			QoSLog.Error("Chickens", "ChickenBalancer failed to initialise: " + ex.Message);
		}
	}

	public void MigrateJsonConfig(VersionSlim savedVersion, Dict<string, object> savedValues) {
		// Config schema v1: nothing to migrate.
	}

	public void Dispose() {
		// The balancer unsubscribes its calendar listener itself if
		// it was resolved; the resolver is not available here, and
		// the listener dies with the session either way.
	}
}
