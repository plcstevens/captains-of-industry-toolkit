using Mafi;

namespace QoSCore;

/// <summary>
/// Suite-wide logging. Every QoS line is prefixed "QoS.&lt;Feature&gt;:"
/// so the suite's output reads as one body in the game log and can be
/// grepped as such. Verbose gates the chatty diagnostics (per-pass
/// skip reasons and the like); it is set once by QoSCoreService from
/// QoSCore's own config and defaults to off.
/// </summary>
public static class QoSLog {

	/// <summary>Chatty-diagnostics switch, set by QoSCoreService.</summary>
	public static bool Verbose;

	public static void Info(string feature, string message) {
		Log.Info("QoS." + feature + ": " + message);
	}

	public static void Warning(string feature, string message) {
		Log.Warning("QoS." + feature + ": " + message);
	}

	public static void Error(string feature, string message) {
		Log.Error("QoS." + feature + ": " + message);
	}

	/// <summary>Info-level line emitted only when Verbose is on.</summary>
	public static void Debug(string feature, string message) {
		if (Verbose) {
			Log.Info("QoS." + feature + " (verbose): " + message);
		}
	}
}
