using Mafi;

namespace QoSCore;

/// <summary>
/// QoSCore's own global dependency: applies core config (the
/// verbose-logging flag) and announces QoSCore itself to the suite
/// registry. This is the whole of core's runtime behaviour — the
/// rest of core is the static QoSLog helper and the registry.
///
/// The mod instance is constructor-injected (the PriorityManager
/// pattern): that is how a service reaches its own mod's
/// ModJsonConfig.
/// </summary>
[GlobalDependency(RegistrationMode.AsEverything)]
public sealed class QoSCoreService {

	public QoSCoreService(QoSCoreMod mod, SuiteRegistry registry) {
		QoSLog.Verbose = mod.JsonConfig.GetBool("verbose_logging", false);
		registry.Announce("QoSCore", QoSCoreMod.Version);
		QoSLog.Info("Core", "QoSCore initialised (verbose logging " +
			(QoSLog.Verbose ? "on" : "off") + ")");
	}
}
