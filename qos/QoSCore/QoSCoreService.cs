using Mafi;
using Mafi.Core.Simulation;

namespace QoSCore;

/// <summary>
/// QoSCore's own global dependency: applies core config (the
/// verbose-logging flag, the activity-log master switch) and
/// announces QoSCore itself to the suite registry. It also wires
/// the activity recorder's game-date provider to the game's
/// verified public clock (ICalendar.CurrentDate — reflection-
/// verified against the 0.8.8 assemblies; see bridge
/// MODDING-NOTES). The rest of core is the static QoSLog helper,
/// the static QoSEventLog recorder and the registry.
///
/// The mod instance is constructor-injected (the PriorityManager
/// pattern): that is how a service reaches its own mod's
/// ModJsonConfig.
/// </summary>
[GlobalDependency(RegistrationMode.AsEverything)]
public sealed class QoSCoreService {

	public QoSCoreService(QoSCoreMod mod, SuiteRegistry registry, ICalendar calendar) {
		QoSLog.Verbose = mod.JsonConfig.GetBool("verbose_logging", false);
		QoSEventLog.Enabled = mod.JsonConfig.GetBool("activity_log_enabled", true);
		if (calendar != null) {
			QoSEventLog.GameDateProvider = delegate {
				GameDate d = calendar.CurrentDate;
				return d.Year + "-" + d.Month + "-" + d.Day;
			};
		}
		registry.Announce("QoSCore", QoSCoreMod.Version);
		QoSLog.Info("Core", "QoSCore initialised (verbose logging " +
			(QoSLog.Verbose ? "on" : "off") + "; activity log " +
			(QoSEventLog.Enabled ? "on" : "off") + ")");
	}
}
