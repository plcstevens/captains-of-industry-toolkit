using System;
using System.Collections.Generic;
using System.Globalization;
using System.Security.Cryptography;
using System.Text;

namespace QoSBuilder;

/// <summary>
/// Job-file JSON support for QoSBuilder: a small self-contained
/// parser (the suite adds no dependencies for fixed, tiny schemas)
/// plus the canonical-payload builder whose SHA-256 the companion
/// signs each job with.
///
/// Job schema v1 (SPEC Addendum XVII) — one JSON object:
///   jobId            string, unique per job (the single-use key)
///   schema           1
///   createdUtc       ISO-8601 UTC string (informational)
///   expiresUtc       ISO-8601 UTC string (hard deadline)
///   gameId           the game-info gameId this job is pinned to
///   companionUser    companion session user who confirmed (info)
///   blueprintName    display name (informational)
///   blueprintString  the blueprint string (B####:...) to place
///   anchor           {x, y, z} — world tile the blueprint's local
///                    origin lands on
///   rotationDeg      0 | 90 | 180 | 270 (counterclockwise steps)
///   applyConfiguration  bool — batch command flag (recipes,
///                    storage products, priorities ride along)
///   limits           {maxItems} — placement cap agreed at confirm
///   sha256           hex SHA-256 of the canonical payload (below)
///
/// The canonical payload is the job re-serialised from its parsed
/// fields in the fixed field order above (sha256 itself excluded),
/// compact, integers bare, strings escaped by ONE rule: '"' as \",
/// '\' as \\, control chars &lt; 0x20 as \u00xx (lowercase hex);
/// everything else raw UTF-8. The companion builds the identical
/// string (its test suite pins a shared test vector), so both sides
/// hash the same bytes or the job is refused as tampered.
/// </summary>
public static class BuilderJson {

	// ------------------------------------------------------------------
	// Canonical payload + hash
	// ------------------------------------------------------------------

	public static string Escape(string s) {
		if (s == null) {
			return "";
		}
		var sb = new StringBuilder(s.Length + 8);
		foreach (char c in s) {
			if (c == '"') {
				sb.Append("\\\"");
			} else if (c == '\\') {
				sb.Append("\\\\");
			} else if (c < 0x20) {
				sb.Append("\\u");
				sb.Append(((int)c).ToString("x4", CultureInfo.InvariantCulture));
			} else {
				sb.Append(c);
			}
		}
		return sb.ToString();
	}

	/// <summary>The canonical payload string for a parsed job's fields.</summary>
	public static string Canonical(string jobId, long schema, string createdUtc, string expiresUtc,
			string gameId, string companionUser, string blueprintName, string blueprintString,
			long anchorX, long anchorY, long anchorZ, long rotationDeg, bool applyConfiguration,
			long maxItems) {
		var sb = new StringBuilder();
		sb.Append("{\"jobId\":\"").Append(Escape(jobId)).Append('"');
		sb.Append(",\"schema\":").Append(schema.ToString(CultureInfo.InvariantCulture));
		sb.Append(",\"createdUtc\":\"").Append(Escape(createdUtc)).Append('"');
		sb.Append(",\"expiresUtc\":\"").Append(Escape(expiresUtc)).Append('"');
		sb.Append(",\"gameId\":\"").Append(Escape(gameId)).Append('"');
		sb.Append(",\"companionUser\":\"").Append(Escape(companionUser)).Append('"');
		sb.Append(",\"blueprintName\":\"").Append(Escape(blueprintName)).Append('"');
		sb.Append(",\"blueprintString\":\"").Append(Escape(blueprintString)).Append('"');
		sb.Append(",\"anchor\":{\"x\":").Append(anchorX.ToString(CultureInfo.InvariantCulture));
		sb.Append(",\"y\":").Append(anchorY.ToString(CultureInfo.InvariantCulture));
		sb.Append(",\"z\":").Append(anchorZ.ToString(CultureInfo.InvariantCulture)).Append('}');
		sb.Append(",\"rotationDeg\":").Append(rotationDeg.ToString(CultureInfo.InvariantCulture));
		sb.Append(",\"applyConfiguration\":").Append(applyConfiguration ? "true" : "false");
		sb.Append(",\"limits\":{\"maxItems\":").Append(maxItems.ToString(CultureInfo.InvariantCulture));
		sb.Append("}}");
		return sb.ToString();
	}

	public static string Sha256Hex(string text) {
		using (SHA256 sha = SHA256.Create()) {
			byte[] hash = sha.ComputeHash(Encoding.UTF8.GetBytes(text ?? ""));
			var sb = new StringBuilder(hash.Length * 2);
			foreach (byte b in hash) {
				sb.Append(b.ToString("x2", CultureInfo.InvariantCulture));
			}
			return sb.ToString();
		}
	}

	// ------------------------------------------------------------------
	// Typed accessors over the parsed object
	// ------------------------------------------------------------------

	public static bool GetString(Dictionary<string, object> obj, string key, out string value) {
		value = null;
		object raw;
		if (obj == null || !obj.TryGetValue(key, out raw)) {
			return false;
		}
		value = raw as string;
		return value != null;
	}

	public static bool GetLong(Dictionary<string, object> obj, string key, out long value) {
		value = 0;
		object raw;
		if (obj == null || !obj.TryGetValue(key, out raw)) {
			return false;
		}
		if (raw is long) {
			value = (long)raw;
			return true;
		}
		if (raw is double) {
			double d = (double)raw;
			if (d == Math.Floor(d) && Math.Abs(d) < 9.0e15) {
				value = (long)d;
				return true;
			}
		}
		return false;
	}

	public static bool GetBool(Dictionary<string, object> obj, string key, out bool value) {
		value = false;
		object raw;
		if (obj == null || !obj.TryGetValue(key, out raw)) {
			return false;
		}
		if (raw is bool) {
			value = (bool)raw;
			return true;
		}
		return false;
	}

	public static bool GetObject(Dictionary<string, object> obj, string key,
			out Dictionary<string, object> value) {
		value = null;
		object raw;
		if (obj == null || !obj.TryGetValue(key, out raw)) {
			return false;
		}
		value = raw as Dictionary<string, object>;
		return value != null;
	}

	// ------------------------------------------------------------------
	// Minimal JSON parser (objects / arrays / strings / numbers /
	// bools / null). Integral numbers come back as long, others as
	// double. Throws FormatException on anything malformed.
	// ------------------------------------------------------------------

	public static Dictionary<string, object> ParseObject(string text) {
		if (text == null) {
			throw new FormatException("null job text");
		}
		var p = new Parser(text);
		p.SkipWs();
		object v = p.ParseValue();
		p.SkipWs();
		if (!p.AtEnd) {
			throw new FormatException("trailing characters after JSON value");
		}
		var obj = v as Dictionary<string, object>;
		if (obj == null) {
			throw new FormatException("job root is not a JSON object");
		}
		return obj;
	}

	private sealed class Parser {
		private readonly string m_text;
		private int m_pos;

		public Parser(string text) {
			m_text = text;
		}

		public bool AtEnd {
			get { return m_pos >= m_text.Length; }
		}

		public void SkipWs() {
			while (m_pos < m_text.Length) {
				char c = m_text[m_pos];
				if (c == ' ' || c == '\t' || c == '\n' || c == '\r') {
					m_pos++;
				} else {
					break;
				}
			}
		}

		private char Peek() {
			if (m_pos >= m_text.Length) {
				throw new FormatException("unexpected end of JSON");
			}
			return m_text[m_pos];
		}

		public object ParseValue() {
			SkipWs();
			char c = Peek();
			if (c == '{') return ParseObjectValue();
			if (c == '[') return ParseArrayValue();
			if (c == '"') return ParseStringValue();
			if (c == 't') { Expect("true"); return true; }
			if (c == 'f') { Expect("false"); return false; }
			if (c == 'n') { Expect("null"); return null; }
			return ParseNumberValue();
		}

		private void Expect(string literal) {
			if (m_pos + literal.Length > m_text.Length ||
					string.Compare(m_text, m_pos, literal, 0, literal.Length, false) != 0) {
				throw new FormatException("expected '" + literal + "'");
			}
			m_pos += literal.Length;
		}

		private Dictionary<string, object> ParseObjectValue() {
			var obj = new Dictionary<string, object>();
			m_pos++; // {
			SkipWs();
			if (Peek() == '}') {
				m_pos++;
				return obj;
			}
			while (true) {
				SkipWs();
				string key = ParseStringValue();
				SkipWs();
				if (Peek() != ':') {
					throw new FormatException("expected ':' in object");
				}
				m_pos++;
				obj[key] = ParseValue();
				SkipWs();
				char c = Peek();
				if (c == ',') {
					m_pos++;
					continue;
				}
				if (c == '}') {
					m_pos++;
					return obj;
				}
				throw new FormatException("expected ',' or '}' in object");
			}
		}

		private List<object> ParseArrayValue() {
			var list = new List<object>();
			m_pos++; // [
			SkipWs();
			if (Peek() == ']') {
				m_pos++;
				return list;
			}
			while (true) {
				list.Add(ParseValue());
				SkipWs();
				char c = Peek();
				if (c == ',') {
					m_pos++;
					continue;
				}
				if (c == ']') {
					m_pos++;
					return list;
				}
				throw new FormatException("expected ',' or ']' in array");
			}
		}

		private string ParseStringValue() {
			if (Peek() != '"') {
				throw new FormatException("expected string");
			}
			m_pos++;
			var sb = new StringBuilder();
			while (true) {
				if (m_pos >= m_text.Length) {
					throw new FormatException("unterminated string");
				}
				char c = m_text[m_pos++];
				if (c == '"') {
					return sb.ToString();
				}
				if (c == '\\') {
					if (m_pos >= m_text.Length) {
						throw new FormatException("unterminated escape");
					}
					char e = m_text[m_pos++];
					switch (e) {
						case '"': sb.Append('"'); break;
						case '\\': sb.Append('\\'); break;
						case '/': sb.Append('/'); break;
						case 'b': sb.Append('\b'); break;
						case 'f': sb.Append('\f'); break;
						case 'n': sb.Append('\n'); break;
						case 'r': sb.Append('\r'); break;
						case 't': sb.Append('\t'); break;
						case 'u': {
							if (m_pos + 4 > m_text.Length) {
								throw new FormatException("short \\u escape");
							}
							int code;
							if (!int.TryParse(m_text.Substring(m_pos, 4),
									NumberStyles.HexNumber, CultureInfo.InvariantCulture, out code)) {
								throw new FormatException("bad \\u escape");
							}
							m_pos += 4;
							sb.Append((char)code);
							break;
						}
						default:
							throw new FormatException("bad escape '\\" + e + "'");
					}
				} else {
					sb.Append(c);
				}
			}
		}

		private object ParseNumberValue() {
			int start = m_pos;
			if (m_pos < m_text.Length && m_text[m_pos] == '-') {
				m_pos++;
			}
			bool isDouble = false;
			while (m_pos < m_text.Length) {
				char c = m_text[m_pos];
				if (c >= '0' && c <= '9') {
					m_pos++;
				} else if (c == '.' || c == 'e' || c == 'E' || c == '+' || c == '-') {
					isDouble = true;
					m_pos++;
				} else {
					break;
				}
			}
			if (m_pos == start) {
				throw new FormatException("expected a JSON value");
			}
			string raw = m_text.Substring(start, m_pos - start);
			if (!isDouble) {
				long l;
				if (long.TryParse(raw, NumberStyles.Integer, CultureInfo.InvariantCulture, out l)) {
					return l;
				}
			}
			double d;
			if (double.TryParse(raw, NumberStyles.Float, CultureInfo.InvariantCulture, out d)) {
				return d;
			}
			throw new FormatException("bad number '" + raw + "'");
		}
	}
}
