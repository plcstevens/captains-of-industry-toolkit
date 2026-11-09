using System;
using Mafi;
using Mafi.Collections;
using Mafi.Core.Game;
using Mafi.Core.Mods;
using Mafi.Core.Prototypes;

namespace QoSCore;

/// <summary>
/// QoSCore — the shared sub-mod of the QoS suite. Feature mods
/// (QoSChickens, and later QoSWorldMap / QoSTrainCargo) declare a
/// manifest dependency on it ("QoSCore >= 0.2.0") and use its
/// logging + suite registry. Core itself changes no gameplay.
///
/// Shape: the plain IMod contract exactly as shipped by
/// CoI_PriorityManager (mod instance constructor-injected into its
/// services, JsonConfig created in the mod constructor). Core
/// registers NO prototypes — removing the folder removes core,
/// and dependent features then fail the game's dependency check
/// cleanly rather than corrupting anything.
/// </summary>
public sealed class QoSCoreMod : IMod {

	/// <summary>Suite version of this mod. Keep in step with manifest.json (manifest is the truth).</summary>
	public const string Version = "0.2.0";

	public QoSCoreMod(ModManifest manifest) {
		Manifest = manifest;
		JsonConfig = new ModJsonConfig(this);
	}

	public ModManifest Manifest { get; }

	public bool IsUiOnly { get { return false; } }

	public Option<IConfig> ModConfig { get { return Option<IConfig>.None; } }

	public ModJsonConfig JsonConfig { get; }

	public void RegisterPrototypes(ProtoRegistrator registrator) {
		// Nothing: core registers no content, by design (removal safety).
	}

	public void RegisterDependencies(DependencyResolverBuilder depBuilder, ProtosDb protosDb, bool gameWasLoaded) {
		depBuilder.RegisterDependency<SuiteRegistry>().AsSelf();
		depBuilder.RegisterDependency<QoSCoreService>().AsSelf();
	}

	public void EarlyInit(DependencyResolver resolver) { }

	public void Initialize(DependencyResolver resolver, bool gameWasLoaded) {
		// Resolve in a try/catch so a core failure is a log line, not
		// a crashed mod load. Feature mods resolve what they need
		// themselves; core failing only costs the suite its logging
		// conveniences, and that must be visible, not silent.
		try {
			resolver.Resolve<QoSCoreService>();
		} catch (Exception ex) {
			Log.Error("QoS.Core: QoSCoreService failed to initialise: " + ex.Message);
		}
	}

	public void MigrateJsonConfig(VersionSlim savedVersion, Dict<string, object> savedValues) {
		// Config schema v1: nothing to migrate.
	}

	public void Dispose() { }
}
