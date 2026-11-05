using System;
using Mafi;
using Mafi.Collections;
using Mafi.Core.Game;
using Mafi.Core.Mods;
using Mafi.Core.Prototypes;
using QoSCore;

namespace QoSVehicleRecovery;

/// <summary>
/// QoSVehicleRecovery — stuck-truck cargo rescue for the QoS suite
/// (QOL-MODS.md items 6-8; design in qos/BUILD-NOTES.md round 3).
/// A policy layer over the game's OWN vehicle recovery and cargo
/// discard commands: it never moves cargo itself, it schedules
/// RecoverVehicleCmd / DiscardVehicleCargoCmd through
/// IInputScheduler when a truck is stuck with its load.
///
/// Depends on QoSCore (manifest: "QoSCore >= 0.1.0") for suite
/// logging + the suite registry; the QoSCore.dll reference is
/// provided by the QoSCore mod folder at runtime and is NOT copied
/// into this mod's folder. Registers NO prototypes — pure
/// behaviour over shipped game systems, so removing the folder
/// removes the mod from any save.
/// </summary>
public sealed class QoSVehicleRecoveryMod : IMod {

	/// <summary>Mod version. Keep in step with manifest.json (manifest is the truth).</summary>
	public const string Version = "0.1.1";

	public QoSVehicleRecoveryMod(ModManifest manifest) {
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
		depBuilder.RegisterDependency<VehicleRecoveryService>().AsSelf();
	}

	public void EarlyInit(DependencyResolver resolver) { }

	public void Initialize(DependencyResolver resolver, bool gameWasLoaded) {
		// Announce to the suite registry (QoSCore). The manifest
		// dependency guarantees QoSCore loaded first; if the
		// registry still cannot resolve, log it and carry on — the
		// service does not depend on the registry to function.
		try {
			SuiteRegistry registry;
			if (resolver.TryResolve<SuiteRegistry>(out registry)) {
				registry.Announce("QoSVehicleRecovery", Version);
			} else {
				QoSLog.Warning("VehicleRecovery", "SuiteRegistry not resolvable — QoSCore loaded?");
			}
		} catch (Exception ex) {
			QoSLog.Error("VehicleRecovery", "suite announce failed: " + ex.Message);
		}
		try {
			resolver.Resolve<VehicleRecoveryService>();
			QoSLog.Info("VehicleRecovery", "QoSVehicleRecovery initialised");
		} catch (Exception ex) {
			QoSLog.Error("VehicleRecovery", "VehicleRecoveryService failed to initialise: " + ex.Message);
		}
	}

	public void MigrateJsonConfig(VersionSlim savedVersion, Dict<string, object> savedValues) {
		// Config schema v1: nothing to migrate.
	}

	public void Dispose() {
		// The service unsubscribes its calendar listener itself if
		// it was resolved; the listener dies with the session
		// either way.
	}
}
