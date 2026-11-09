using System;
using Mafi;
using Mafi.Collections;
using Mafi.Core.Game;
using Mafi.Core.Mods;
using Mafi.Core.Prototypes;
using QoSCore;

namespace QoSPriorityActuator;

/// <summary>
/// QoSPriorityActuator — bounded priority nudges for starved
/// machines, for the QoS suite (PATTERN-SURVEY.md §2.3; design in
/// qos/BUILD-NOTES.md round 5). It is the actuator counterpart to
/// the companion's productivity estimators (QOL-MOD-PREP.md §5):
/// the companion DETECTS with evidence classes, this mod ACTS with
/// bounded steps, and both share §5's cause vocabulary (STARVED /
/// BLOCKED / PARKED). A policy layer over the game's OWN
/// SetGeneralPriorityCmd, scheduled through IInputScheduler: it
/// never writes an entity value directly.
///
/// The heart of the mod is a STATELESS ownership rule: it only
/// ever adjusts a machine whose current general priority is a
/// value the mod's own rule could have produced (the proto
/// default, or the default minus up to the configured step band).
/// Anything else is treated as player-set and is never fought,
/// and restoring a machine means returning it to its proto
/// default under the same test. No per-entity memory is needed
/// for safety, so nothing about the rule degrades across reloads.
///
/// Depends on QoSCore (manifest: "QoSCore >= 0.1.0") for suite
/// logging + the suite registry; the QoSCore.dll reference is
/// provided by the QoSCore mod folder at runtime and is NOT
/// copied into this mod's folder. Registers NO prototypes —
/// pure behaviour over shipped game systems and registers no
/// content. NOTE (0.1.2): the service's calendar subscription
/// IS serialized into saves written while the mod is installed
/// (the sanctioned serializer pair on PriorityActuatorService),
/// so the manifest sets can_remove_from_saved_game=false — the
/// game blocks removing the mod from such a save rather than
/// letting the save become unloadable.
/// </summary>
public sealed class QoSPriorityActuatorMod : IMod {

	/// <summary>Mod version. Keep in step with manifest.json (manifest is the truth).</summary>
	public const string Version = "0.1.2";

	public QoSPriorityActuatorMod(ModManifest manifest) {
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
		depBuilder.RegisterDependency<PriorityActuatorService>().AsSelf();
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
				registry.Announce("QoSPriorityActuator", Version);
			} else {
				QoSLog.Warning("PriorityActuator", "SuiteRegistry not resolvable — QoSCore loaded?");
			}
		} catch (Exception ex) {
			QoSLog.Error("PriorityActuator", "suite announce failed: " + ex.Message);
		}
		try {
			resolver.Resolve<PriorityActuatorService>();
			QoSLog.Info("PriorityActuator", "QoSPriorityActuator initialised");
		} catch (Exception ex) {
			QoSLog.Error("PriorityActuator", "PriorityActuatorService failed to initialise: " + ex.Message);
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
