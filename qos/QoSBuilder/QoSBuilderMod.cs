using System;
using Mafi;
using Mafi.Collections;
using Mafi.Core.Game;
using Mafi.Core.Mods;
using Mafi.Core.Prototypes;
using QoSCore;

namespace QoSBuilder;

/// <summary>
/// QoSBuilder — the QoS suite's placement executor (recon 10 §Q5,
/// approved by Philip 2026-11-05; probe in MODDING-NOTES round 12).
/// The web /command channel can never carry BuildTransportCmd or
/// BatchCreateStaticEntitiesCmd (registry-only JSON index), so
/// blueprint / belt / pipe placement reaches the game through this
/// mod instead: the CoI Companion website writes a job file ONLY at
/// its §12 confirm step, and this mod executes the job — once, if
/// it is unexpired, untampered and pinned to the running game — by
/// scheduling the game's own BatchCreateStaticEntitiesCmd through
/// IInputScheduler. See BuilderService for the execution contract
/// and readme.txt for the plain-words version.
///
/// Depends on QoSCore (manifest: "QoSCore >= 0.2.0") for suite
/// logging + the activity recorder; the QoSCore.dll reference is
/// provided by the QoSCore mod folder at runtime and is NOT copied
/// into this mod's folder. Registers NO prototypes: placement
/// creates ordinary construction sites through a shipped command.
/// </summary>
public sealed class QoSBuilderMod : IMod {

	/// <summary>Mod version. Keep in step with manifest.json (manifest is the truth).</summary>
	public const string Version = "0.1.0";

	public QoSBuilderMod(ModManifest manifest) {
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
		depBuilder.RegisterDependency<BuilderService>().AsSelf();
	}

	public void EarlyInit(DependencyResolver resolver) { }

	public void Initialize(DependencyResolver resolver, bool gameWasLoaded) {
		// Announce to the suite registry (QoSCore). The manifest
		// dependency guarantees QoSCore loaded first; if the
		// registry still cannot resolve, log it and carry on — the
		// builder does not depend on the registry to function.
		try {
			SuiteRegistry registry;
			if (resolver.TryResolve<SuiteRegistry>(out registry)) {
				registry.Announce("QoSBuilder", Version);
			} else {
				QoSLog.Warning("Builder", "SuiteRegistry not resolvable — QoSCore loaded?");
			}
		} catch (Exception ex) {
			QoSLog.Error("Builder", "suite announce failed: " + ex.Message);
		}
		try {
			resolver.Resolve<BuilderService>();
			QoSLog.Info("Builder", "QoSBuilder initialised");
		} catch (Exception ex) {
			QoSLog.Error("Builder", "BuilderService failed to initialise: " + ex.Message);
		}
	}

	public void MigrateJsonConfig(VersionSlim savedVersion, Dict<string, object> savedValues) {
		// Config schema v1: nothing to migrate.
	}

	public void Dispose() {
		// The service unsubscribes its calendar listener itself if
		// it was resolved; the resolver is not available here, and
		// the listener dies with the session either way.
	}
}
