using System;
using System.Collections.Generic;
using System.Reflection;
using System.Text;

namespace CoiBridge;

/// <summary>
/// Runtime discovery report for CoiBridge M1.1.
///
/// The bridge was written without access to the game assemblies on the
/// development machine, so the exact M2 backing services (research, trains,
/// census, storages, settlement) are still marked GUESS in BridgeRoutes.cs.
/// This report is the discovery-first workaround: when the mod runs inside the
/// real game, it uses plain System.Reflection metadata to describe — never
/// invoke — the dependency resolver it was handed plus the loaded Mafi /
/// Assembly-CSharp types whose names match the M2 service families. That is
/// enough to pin the real service names and public read surfaces for the next
/// build without shipping or redistributing the game's DLLs.
///
/// Safety notes:
///   * Reflection here reads metadata only. No discovered game method or
///     property is called, and no game object is mutated.
///   * The report is cached after the first build; route handlers can be hit
///     repeatedly without rescanning assemblies.
///   * Output is capped. This is a targeting report, not an assembly dump.
/// </summary>
public static class DiscoveryReport {

	private static readonly object s_lock = new object();
	private static object s_resolver;
	private static string s_cachedJson;

	private const int MaxCandidateTypes = 220;
	private const int MaxMethodsPerType = 18;
	private const int MaxPropertiesPerType = 12;
	private const int MaxFieldsPerType = 8;
	private const int MaxInterfacesPerType = 8;

	public static void CaptureResolver(object resolver) {
		lock (s_lock) {
			s_resolver = resolver;
			s_cachedJson = null;
		}
	}

	public static string Build(string[] routeTable, string modVersion, string gameApi) {
		lock (s_lock) {
			if (s_cachedJson != null) return s_cachedJson;
			s_cachedJson = BuildInternal(routeTable, modVersion, gameApi);
			return s_cachedJson;
		}
	}

	private static string BuildInternal(string[] routeTable, string modVersion, string gameApi) {
		var sb = new StringBuilder();
		sb.Append("{\"mod\":\"CoiBridge\",\"version\":\"").Append(JsonEscape(modVersion))
			.Append("\",\"gameApi\":\"").Append(JsonEscape(gameApi))
			.Append("\",\"purpose\":\"runtime discovery metadata only; no game methods are invoked and no game state is mutated\"")
			.Append(",\"bridgeAssembly\":\"").Append(JsonEscape(typeof(DiscoveryReport).Assembly.GetName().Name))
			.Append("\",\"routes\":");
		AppendStringArray(sb, routeTable);
		sb.Append(",\"resolver\":");
		AppendResolverJson(sb, s_resolver);
		sb.Append(",\"assemblies\":");
		AppendAssembliesJson(sb);
		sb.Append(",\"candidateTypes\":");
		bool truncated;
		AppendCandidateTypesJson(sb, out truncated);
		sb.Append(",\"candidateTypesTruncated\":").Append(truncated ? "true" : "false");
		sb.Append(",\"limits\":{\"maxCandidateTypes\":").Append(MaxCandidateTypes)
			.Append(",\"maxMethodsPerType\":").Append(MaxMethodsPerType)
			.Append(",\"maxPropertiesPerType\":").Append(MaxPropertiesPerType)
			.Append(",\"maxFieldsPerType\":").Append(MaxFieldsPerType)
			.Append('}');
		sb.Append('}');
		return sb.ToString();
	}

	private static void AppendResolverJson(StringBuilder sb, object resolver) {
		if (resolver == null) {
			sb.Append("{\"captured\":false}");
			return;
		}
		Type type = resolver.GetType();
		sb.Append("{\"captured\":true,\"type\":\"").Append(JsonEscape(TypeName(type)))
			.Append("\",\"assembly\":\"").Append(JsonEscape(type.Assembly.GetName().Name))
			.Append("\",\"methods\":");
		AppendMethodArray(sb, type, MaxMethodsPerType * 2);
		sb.Append(",\"properties\":");
		AppendPropertyArray(sb, type, MaxPropertiesPerType * 2);
		sb.Append(",\"fields\":");
		AppendFieldArray(sb, type, MaxFieldsPerType * 2);
		sb.Append('}');
	}

	private static void AppendAssembliesJson(StringBuilder sb) {
		var assemblies = new List<Assembly>();
		foreach (Assembly assembly in AppDomain.CurrentDomain.GetAssemblies()) {
			if (IsRelevantAssembly(assembly)) assemblies.Add(assembly);
		}
		assemblies.Sort((a, b) => string.Compare(a.GetName().Name, b.GetName().Name, StringComparison.Ordinal));
		sb.Append('[');
		for (int i = 0; i < assemblies.Count; i++) {
			if (i > 0) sb.Append(',');
			AssemblyName name = assemblies[i].GetName();
			sb.Append("{\"name\":\"").Append(JsonEscape(name.Name))
				.Append("\",\"version\":\"").Append(JsonEscape(name.Version == null ? "" : name.Version.ToString()))
				.Append("\"}");
		}
		sb.Append(']');
	}

	private static void AppendCandidateTypesJson(StringBuilder sb, out bool truncated) {
		var types = new List<Type>();
		foreach (Assembly assembly in AppDomain.CurrentDomain.GetAssemblies()) {
			if (!IsRelevantAssembly(assembly)) continue;
			foreach (Type type in GetLoadableTypes(assembly)) {
				if (type != null && IsCandidateType(type)) types.Add(type);
			}
		}
		types.Sort((a, b) => {
			int byAssembly = string.Compare(a.Assembly.GetName().Name, b.Assembly.GetName().Name, StringComparison.Ordinal);
			return byAssembly != 0 ? byAssembly : string.Compare(TypeName(a), TypeName(b), StringComparison.Ordinal);
		});
		truncated = types.Count > MaxCandidateTypes;
		int limit = Math.Min(types.Count, MaxCandidateTypes);
		sb.Append('[');
		for (int i = 0; i < limit; i++) {
			if (i > 0) sb.Append(',');
			AppendTypeJson(sb, types[i]);
		}
		sb.Append(']');
	}

	private static void AppendTypeJson(StringBuilder sb, Type type) {
		sb.Append("{\"assembly\":\"").Append(JsonEscape(type.Assembly.GetName().Name))
			.Append("\",\"type\":\"").Append(JsonEscape(TypeName(type)))
			.Append("\",\"kind\":\"").Append(type.IsInterface ? "interface" : type.IsEnum ? "enum" : type.IsValueType ? "struct" : "class")
			.Append("\",\"base\":\"").Append(JsonEscape(type.BaseType == null ? "" : TypeName(type.BaseType)))
			.Append("\",\"interfaces\":");
		AppendInterfaceArray(sb, type);
		sb.Append(",\"methods\":");
		AppendMethodArray(sb, type, MaxMethodsPerType);
		sb.Append(",\"properties\":");
		AppendPropertyArray(sb, type, MaxPropertiesPerType);
		sb.Append(",\"fields\":");
		AppendFieldArray(sb, type, MaxFieldsPerType);
		sb.Append('}');
	}

	private static IEnumerable<Type> GetLoadableTypes(Assembly assembly) {
		try {
			return assembly.GetTypes();
		} catch (ReflectionTypeLoadException ex) {
			var readable = new List<Type>();
			foreach (Type type in ex.Types) {
				if (type != null) readable.Add(type);
			}
			return readable;
		} catch {
			return new Type[0];
		}
	}

	private static bool IsRelevantAssembly(Assembly assembly) {
		string name = assembly.GetName().Name ?? string.Empty;
		return name == "Assembly-CSharp"
			|| name.StartsWith("Mafi", StringComparison.Ordinal)
			|| name.IndexOf("Train", StringComparison.OrdinalIgnoreCase) >= 0;
	}

	private static bool IsCandidateType(Type type) {
		string fullName = TypeName(type);
		string n = fullName.ToLowerInvariant();
		if (n.Contains("webapi") || n.Contains("webroute") || n.Contains("gameapi") || n.Contains("dependencyresolver")) return true;
		if (n.Contains("research") && (n.Contains("manager") || n.Contains("service") || n.Contains("db") || n.Contains("system"))) return true;
		if (n.Contains("train") && (n.Contains("manager") || n.Contains("service") || n.Contains("network") || n.Contains("statistic") || n.Contains("stats"))) return true;
		if (n.Contains("storage") && (n.Contains("manager") || n.Contains("service") || n.Contains("buffer"))) return true;
		if (n.Contains("population")) return true;
		if (n.Contains("settlement") && (n.Contains("manager") || n.Contains("service") || n.Contains("system"))) return true;
		if (n.Contains("entitymanager") || n.Contains("entitiesmanager") || n.Contains("entitiesdb") || n.Contains("protosdb")) return true;
		return false;
	}

	private static void AppendMethodArray(StringBuilder sb, Type type, int limit) {
		var values = new List<string>();
		try {
			MethodInfo[] methods = type.GetMethods(BindingFlags.Public | BindingFlags.Instance | BindingFlags.Static | BindingFlags.DeclaredOnly);
			Array.Sort(methods, (a, b) => string.Compare(a.Name, b.Name, StringComparison.Ordinal));
			foreach (MethodInfo method in methods) {
				if (method.IsSpecialName) continue;
				values.Add(Safe(() => method.ToString()));
				if (values.Count >= limit) break;
			}
		} catch {
			values.Add("<unavailable>");
		}
		AppendStringArray(sb, values.ToArray());
	}

	private static void AppendPropertyArray(StringBuilder sb, Type type, int limit) {
		var values = new List<string>();
		try {
			PropertyInfo[] properties = type.GetProperties(BindingFlags.Public | BindingFlags.Instance | BindingFlags.Static | BindingFlags.DeclaredOnly);
			Array.Sort(properties, (a, b) => string.Compare(a.Name, b.Name, StringComparison.Ordinal));
			foreach (PropertyInfo property in properties) {
				values.Add(Safe(() => TypeName(property.PropertyType) + " " + property.Name));
				if (values.Count >= limit) break;
			}
		} catch {
			values.Add("<unavailable>");
		}
		AppendStringArray(sb, values.ToArray());
	}

	private static void AppendFieldArray(StringBuilder sb, Type type, int limit) {
		var values = new List<string>();
		try {
			FieldInfo[] fields = type.GetFields(BindingFlags.Public | BindingFlags.Instance | BindingFlags.Static | BindingFlags.DeclaredOnly);
			Array.Sort(fields, (a, b) => string.Compare(a.Name, b.Name, StringComparison.Ordinal));
			foreach (FieldInfo field in fields) {
				values.Add(Safe(() => TypeName(field.FieldType) + " " + field.Name));
				if (values.Count >= limit) break;
			}
		} catch {
			values.Add("<unavailable>");
		}
		AppendStringArray(sb, values.ToArray());
	}

	private static void AppendInterfaceArray(StringBuilder sb, Type type) {
		var values = new List<string>();
		try {
			Type[] interfaces = type.GetInterfaces();
			Array.Sort(interfaces, (a, b) => string.Compare(TypeName(a), TypeName(b), StringComparison.Ordinal));
			foreach (Type iface in interfaces) {
				values.Add(TypeName(iface));
				if (values.Count >= MaxInterfacesPerType) break;
			}
		} catch {
			values.Add("<unavailable>");
		}
		AppendStringArray(sb, values.ToArray());
	}

	private static void AppendStringArray(StringBuilder sb, string[] values) {
		sb.Append('[');
		for (int i = 0; i < values.Length; i++) {
			if (i > 0) sb.Append(',');
			sb.Append('\"').Append(JsonEscape(values[i])).Append('\"');
		}
		sb.Append(']');
	}

	private static string TypeName(Type type) {
		return Safe(() => type.FullName ?? type.Name);
	}

	private static string Safe(Func<string> read) {
		try {
			return read() ?? string.Empty;
		} catch (Exception ex) {
			return "<error:" + ex.GetType().Name + ">";
		}
	}

	private static string JsonEscape(string value) {
		if (string.IsNullOrEmpty(value)) return string.Empty;
		var sb = new StringBuilder(value.Length);
		foreach (char c in value) {
			switch (c) {
				case '\"': sb.Append("\\\""); break;
				case '\\': sb.Append("\\\\"); break;
				case '\n': sb.Append("\\n"); break;
				case '\r': sb.Append("\\r"); break;
				case '\t': sb.Append("\\t"); break;
				default:
					if (c < ' ') sb.Append("\\u").Append(((int)c).ToString("x4"));
					else sb.Append(c);
					break;
			}
		}
		return sb.ToString();
	}
}
