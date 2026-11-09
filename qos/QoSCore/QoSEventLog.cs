using System;
using System.Collections;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Text;

namespace QoSCore;

/// <summary>
/// The QoS suite's unified activity recorder (SPEC Addendum XVI).
/// Every ACTUAL action a suite mod takes — a command it scheduled
/// through IInputScheduler, a navigation it performed — is appended
/// here as one JSON line to a feed file next to the game's data:
///
///   %APPDATA%\Captain of Industry\QoS\events-v1.jsonl
///
/// The companion website imports this feed into its activity store
/// and shows it on the Log page next to its own records, so the
/// whole toolkit's doings replay in one chronological list.
///
/// Line schema v1 (one JSON object per line):
///   schema        1
///   eventId       GUID string, unique per event (import dedupe key)
///   recordedAtUtc ISO-8601 UTC wall-clock time of the action
///   source        "qos"
///   modId         e.g. "QoSChickens"
///   modVersion    the acting mod's version
///   kind          "command" | "navigation" | ...
///   action        the shipped command type name, or the action taken
///   summary       one human line, same spirit as the QoSLog line
///   data          structured context (ids, amounts, predictions);
///                 may carry "gameDate" (injected by the provider
///                 below when the caller did not supply one)
///
/// Discipline:
/// - FAIL-SAFE: Record never throws into the simulation. Any I/O or
///   serialisation failure is swallowed (one warning line, once).
/// - Thread-safe: appends are serialised under a lock.
/// - NOT a debug log: callers record actions only, never per-pass
///   diagnostics or skipped no-ops, and recording is not gated by
///   the features' verbosity flags — only by QoSCore's own
///   activity_log_enabled config (default on), wired into
///   <see cref="Enabled"/> by QoSCoreService.
/// - Hand-rolled JSON: no new dependencies for a fixed, tiny schema.
/// </summary>
public static class QoSEventLog {

	/// <summary>Master switch (QoSCore config activity_log_enabled, default true).</summary>
	public static bool Enabled = true;

	/// <summary>
	/// Optional in-game date provider, set by QoSCoreService from
	/// ICalendar.CurrentDate (the game's verified public clock).
	/// Returns the date formatted like the game's own "388-2-27",
	/// or null to record no date. There is deliberately no tick:
	/// ICalendar exposes a date, not a tick.
	/// </summary>
	public static Func<string> GameDateProvider;

	private static readonly object s_lock = new object();
	private static string s_path;
	private static bool s_warned;

	/// <summary>The feed file path (resolved lazily, then fixed).</summary>
	public static string FeedPath {
		get {
			lock (s_lock) {
				return FeedPathLocked();
			}
		}
	}

	private static string FeedPathLocked() {
		if (s_path == null) {
			s_path = Path.Combine(
				Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
				"Captain of Industry", "QoS", "events-v1.jsonl");
		}
		return s_path;
	}

	/// <summary>
	/// Append one activity event. Never throws. Data values may be
	/// string / bool / integral / double / IDictionary&lt;string,
	/// object&gt; / IEnumerable; anything else is recorded as its
	/// ToString() in quotes.
	/// </summary>
	public static void Record(string modId, string modVersion, string kind, string action,
			string summary, IDictionary<string, object> data) {
		if (!Enabled) {
			return;
		}
		try {
			string line = BuildLine(modId, modVersion, kind, action, summary, data);
			lock (s_lock) {
				string path = FeedPathLocked();
				Directory.CreateDirectory(Path.GetDirectoryName(path));
				using (var writer = new StreamWriter(path, true, new UTF8Encoding(false))) {
					writer.WriteLine(line);
				}
			}
		} catch (Exception ex) {
			if (!s_warned) {
				s_warned = true;
				try {
					QoSLog.Warning("Core", "activity log write failed (further failures silent): " +
						ex.Message);
				} catch {
					// Even the warning must never throw into the sim.
				}
			}
		}
	}

	private static string BuildLine(string modId, string modVersion, string kind, string action,
			string summary, IDictionary<string, object> data) {
		// Merge caller data with the provider's gameDate (caller's
		// own value wins) without mutating the caller's dictionary.
		var merged = new Dictionary<string, object>();
		if (data != null) {
			foreach (KeyValuePair<string, object> kv in data) {
				merged[kv.Key] = kv.Value;
			}
		}
		if (!merged.ContainsKey("gameDate") && GameDateProvider != null) {
			string date = null;
			try {
				date = GameDateProvider();
			} catch {
				date = null;
			}
			if (date != null) {
				merged["gameDate"] = date;
			}
		}

		var sb = new StringBuilder();
		sb.Append('{');
		WriteString(sb, "schema");
		sb.Append(":1,");
		WriteString(sb, "eventId");
		sb.Append(':');
		WriteString(sb, Guid.NewGuid().ToString());
		sb.Append(',');
		WriteString(sb, "recordedAtUtc");
		sb.Append(':');
		WriteString(sb, DateTime.UtcNow.ToString("o", CultureInfo.InvariantCulture));
		sb.Append(',');
		WriteString(sb, "source");
		sb.Append(':');
		WriteString(sb, "qos");
		sb.Append(',');
		WriteString(sb, "modId");
		sb.Append(':');
		WriteString(sb, modId ?? "");
		sb.Append(',');
		WriteString(sb, "modVersion");
		sb.Append(':');
		WriteString(sb, modVersion ?? "");
		sb.Append(',');
		WriteString(sb, "kind");
		sb.Append(':');
		WriteString(sb, kind ?? "");
		sb.Append(',');
		WriteString(sb, "action");
		sb.Append(':');
		WriteString(sb, action ?? "");
		sb.Append(',');
		WriteString(sb, "summary");
		sb.Append(':');
		WriteString(sb, summary ?? "");
		sb.Append(',');
		WriteString(sb, "data");
		sb.Append(':');
		WriteValue(sb, merged);
		sb.Append('}');
		return sb.ToString();
	}

	private static void WriteValue(StringBuilder sb, object value) {
		if (value == null) {
			sb.Append("null");
			return;
		}
		string str = value as string;
		if (str != null) {
			WriteString(sb, str);
			return;
		}
		if (value is bool) {
			sb.Append((bool)value ? "true" : "false");
			return;
		}
		var dict = value as IDictionary<string, object>;
		if (dict != null) {
			sb.Append('{');
			bool first = true;
			foreach (KeyValuePair<string, object> kv in dict) {
				if (!first) {
					sb.Append(',');
				}
				first = false;
				WriteString(sb, kv.Key);
				sb.Append(':');
				WriteValue(sb, kv.Value);
			}
			sb.Append('}');
			return;
		}
		var list = value as IEnumerable;
		if (list != null) {
			sb.Append('[');
			bool first = true;
			foreach (object item in list) {
				if (!first) {
					sb.Append(',');
				}
				first = false;
				WriteValue(sb, item);
			}
			sb.Append(']');
			return;
		}
		if (value is double) {
			sb.Append(((double)value).ToString("R", CultureInfo.InvariantCulture));
			return;
		}
		if (value is float) {
			sb.Append(((float)value).ToString("R", CultureInfo.InvariantCulture));
			return;
		}
		if (value is decimal) {
			sb.Append(((decimal)value).ToString(CultureInfo.InvariantCulture));
			return;
		}
		if (value is byte || value is sbyte || value is short || value is ushort ||
				value is int || value is uint || value is long || value is ulong) {
			sb.Append(Convert.ToString(value, CultureInfo.InvariantCulture));
			return;
		}
		WriteString(sb, value.ToString());
	}

	private static void WriteString(StringBuilder sb, string value) {
		sb.Append('"');
		foreach (char c in value) {
			switch (c) {
				case '"': sb.Append("\\\""); break;
				case '\\': sb.Append("\\\\"); break;
				case '\n': sb.Append("\\n"); break;
				case '\r': sb.Append("\\r"); break;
				case '\t': sb.Append("\\t"); break;
				default:
					if (c < 0x20) {
						sb.Append("\\u");
						sb.Append(((int)c).ToString("x4", CultureInfo.InvariantCulture));
					} else {
						sb.Append(c);
					}
					break;
			}
		}
		sb.Append('"');
	}
}
