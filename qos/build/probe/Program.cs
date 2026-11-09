using System;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Collections.Generic;

// QoSBuilder probe (recon round 11): can a mod assembly reach the game's
// blueprint deserialization path? Plus the surfaces the builder needs:
// gameId source, command ctors, transform types, InputCommand state.
// Modes:
//   dump    -> visibility/member survey (no invocation)
//   invoke  -> runtime reachability test against a real pack string
class Program {
	static List<Assembly> Asms = new List<Assembly>();
	static List<Type> AllTypes = new List<Type>();

	static void LoadAll() {
		var dir = "/home/hatch/workspace/coi-companion/refs";
		AppDomain.CurrentDomain.AssemblyResolve += (s, e) => {
			var p = Path.Combine(dir, new AssemblyName(e.Name).Name + ".dll");
			return File.Exists(p) ? Assembly.LoadFrom(p) : null;
		};
		foreach (var f in Directory.GetFiles(dir, "*.dll")) {
			try {
				var asm = Assembly.LoadFrom(f);
				Asms.Add(asm);
				try { AllTypes.AddRange(asm.GetTypes()); }
				catch (ReflectionTypeLoadException ex) { AllTypes.AddRange(ex.Types.Where(t => t != null)); }
			} catch (Exception ex) { Console.WriteLine("LOADFAIL " + f + " " + ex.GetType().Name); }
		}
	}

	static string TN(Type t) { return t == null ? "?" : (t.FullName ?? t.Name); }
	static string Vis(MethodBase m) {
		if (m.IsPublic) return "pub";
		if (m.IsFamily) return "prot";
		if (m.IsAssembly) return "int";
		return "priv";
	}

	static void DumpType(Type t, bool members) {
		Console.WriteLine("TYPE " + TN(t) + " vis=" + (t.IsPublic ? "public" : t.IsNotPublic ? "internal" : "nested") +
			" base=" + TN(t.BaseType) + (t.IsEnum ? " [ENUM]" : "") + (t.IsInterface ? " [INTERFACE]" : "") +
			(t.IsAbstract ? " [abstract]" : "") + (t.IsSealed ? " [sealed]" : ""));
		if (t.IsEnum) { foreach (var n in Enum.GetNames(t)) Console.WriteLine("  enumval " + n); return; }
		if (!members) return;
		const BindingFlags F = BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance | BindingFlags.Static | BindingFlags.DeclaredOnly;
		foreach (var c in t.GetConstructors(F))
			Console.WriteLine("  ctor " + Vis(c) + " (" + string.Join(", ", c.GetParameters().Select(x => TN(x.ParameterType) + " " + x.Name)) + ")");
		foreach (var p in t.GetProperties(F))
			Console.WriteLine("  prop get:" + (p.GetMethod != null ? Vis(p.GetMethod) : "----") + " set:" + (p.SetMethod != null ? Vis(p.SetMethod) : "----") + " " + TN(p.PropertyType) + " " + p.Name);
		foreach (var f in t.GetFields(F))
			Console.WriteLine("  field " + (f.IsPublic ? "pub" : f.IsAssembly ? "int" : f.IsFamily ? "prot" : "priv") + " " + TN(f.FieldType) + " " + f.Name + (f.IsLiteral ? " = " + f.GetRawConstantValue() : ""));
		foreach (var m in t.GetMethods(F).Where(m => !m.IsSpecialName))
			Console.WriteLine("  meth " + Vis(m) + " " + TN(m.ReturnType) + " " + m.Name + "(" + string.Join(", ", m.GetParameters().Select(x => TN(x.ParameterType) + " " + x.Name)) + ")");
		foreach (var e in t.GetEvents(F))
			Console.WriteLine("  event " + TN(e.EventHandlerType) + " " + e.Name);
		foreach (var nt in t.GetNestedTypes(BindingFlags.Public | BindingFlags.NonPublic))
			Console.WriteLine("  nested " + TN(nt) + " vis=" + (nt.IsNestedPublic ? "public" : "nonpub"));
	}

	static Type Find(string fullName) { return AllTypes.FirstOrDefault(x => TN(x) == fullName); }

	static void Main(string[] args) {
		LoadAll();
		Console.WriteLine("TOTAL TYPES " + AllTypes.Count);
		string mode = args.Length > 0 ? args[0] : "dump";

		if (mode == "invoke") { InvokeTest(); return; }
		if (mode.StartsWith("dump:")) {
			var t = Find(mode.Substring(5));
			if (t == null) { Console.WriteLine("NOT FOUND " + mode.Substring(5)); return; }
			DumpType(t, true);
			return;
		}
		if (mode.StartsWith("attr:")) {
			var t = Find(mode.Substring(5));
			if (t == null) { Console.WriteLine("NOT FOUND"); return; }
			try {
				foreach (var a in t.GetCustomAttributesData())
					Console.WriteLine("  attr " + a.AttributeType.FullName + " (" + string.Join(", ", a.ConstructorArguments.Select(x => Convert.ToString(x.Value))) + ")");
			} catch (Exception ex) { Console.WriteLine("attr read failed: " + ex.Message); }
			return;
		}
		if (mode.StartsWith("find:")) {
			foreach (var t in AllTypes.Where(x => x.Name == mode.Substring(5)).OrderBy(x => TN(x)))
				Console.WriteLine(TN(t) + " vis=" + (t.IsPublic ? "public" : "nonpub"));
			return;
		}

		// 1. InternalsVisibleTo on the game assemblies
		foreach (var asm in Asms) {
			try {
				foreach (var attr in asm.GetCustomAttributesData().Where(a => a.AttributeType.Name.Contains("InternalsVisibleTo"))) {
					Console.WriteLine("IVT " + asm.GetName().Name + " -> " + string.Join("", attr.ConstructorArguments.Select(a => a.Value.ToString())));
				}
			} catch { /* UnityEngine attribute blobs reference assemblies we do not load */ }
		}

		// 2. All Blueprint-ish types
		Console.WriteLine("== types with 'Blueprint' in name ==");
		foreach (var t in AllTypes.Where(t => TN(t).Contains("Blueprint")).OrderBy(t => TN(t)))
			DumpType(t, false);

		// 3. Methods anywhere mentioning blueprint deserialization / import
		Console.WriteLine("== methods named *Blueprint* (de)serial/import ==");
		const BindingFlags F = BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance | BindingFlags.Static | BindingFlags.DeclaredOnly;
		foreach (var t in AllTypes) {
			MethodInfo[] ms;
			try { ms = t.GetMethods(F); } catch { continue; }
			foreach (var m in ms.Where(m => !m.IsSpecialName)) {
				string n = m.Name;
				if ((n.Contains("Blueprint") && (n.Contains("Deserial") || n.Contains("Import") || n.Contains("Parse") || n.Contains("FromString") || n.Contains("Decode")))
					|| n == "DeserializeForBlueprints") {
					Console.WriteLine("  " + Vis(m) + " " + TN(t) + "." + n + "(" +
						string.Join(", ", m.GetParameters().Select(x => TN(x.ParameterType) + " " + x.Name)) + ") -> " + TN(m.ReturnType));
				}
			}
		}

		// 4. The key types in full
		foreach (var name in new[] {
			"Mafi.Core.Entities.Blueprints.Blueprint",
		}) {
			var t = Find(name);
			Console.WriteLine("== dump " + name + " ==");
			if (t == null) Console.WriteLine("NOT FOUND"); else DumpType(t, true);
		}
		var ecd = AllTypes.FirstOrDefault(x => x.Name == "EntityConfigData");
		Console.WriteLine("== dump EntityConfigData (" + TN(ecd) + ") ==");
		if (ecd != null) DumpType(ecd, true);

		// 5. game-info web route: find the handler, dump its declaring type
		Console.WriteLine("== WebRoute game-info ==");
		foreach (var t in AllTypes) {
			MethodInfo[] ms;
			try { ms = t.GetMethods(F); } catch { continue; }
			foreach (var m in ms) {
				try {
					foreach (var a in m.GetCustomAttributesData()) {
						if (a.AttributeType.Name == "WebRouteAttribute" &&
							a.ConstructorArguments.Any(x => (x.Value as string) != null && ((string)x.Value).Contains("game-info"))) {
							Console.WriteLine("  route on " + TN(t) + "." + m.Name);
						}
					}
				} catch { /* attribute blob references assemblies we do not load */ }
			}
		}
		// members named GameId anywhere public
		Console.WriteLine("== public members named GameId/GameID ==");
		foreach (var t in AllTypes) {
			try {
				foreach (var p in t.GetProperties(BindingFlags.Public | BindingFlags.Instance | BindingFlags.Static | BindingFlags.DeclaredOnly))
					if (p.Name.Contains("GameId") || p.Name.Contains("GameID")) Console.WriteLine("  prop " + TN(t) + "." + p.Name + " : " + TN(p.PropertyType));
				foreach (var f in t.GetFields(BindingFlags.Public | BindingFlags.Instance | BindingFlags.Static | BindingFlags.DeclaredOnly))
					if (f.Name.Contains("GameId") || f.Name.Contains("GameID")) Console.WriteLine("  field " + TN(t) + "." + f.Name + " : " + TN(f.FieldType));
			} catch { }
		}

		// 6. Command + transform surface dumps
		foreach (var name in new[] {
			"Mafi.Core.Entities.Static.Commands.BatchCreateStaticEntitiesCmd",
			"Mafi.Core.Factory.Transports.BuildTransportCmd",
			"Mafi.Core.Input.InputCommand",
			"Mafi.Core.Entities.TileTransform",
			"Mafi.Core.Geometry.Direction903d",
		}) {
			var t = Find(name);
			Console.WriteLine("== dump " + name + " ==");
			if (t == null) Console.WriteLine("NOT FOUND"); else DumpType(t, true);
		}
		// CreateStaticEntityCmd + Tile3i/2i + Rotation90 + BuildMiniZippersMode by short name
		foreach (var shortName in new[] { "CreateStaticEntityCmd", "Tile3i", "Tile2i", "Rotation90", "BuildMiniZippersMode", "RelTile3i" }) {
			var t = AllTypes.FirstOrDefault(x => x.Name == shortName);
			Console.WriteLine("== dump short " + shortName + " (" + TN(t) + ") ==");
			if (t != null) DumpType(t, true);
		}
		// InputCommand<T>
		var icg = AllTypes.FirstOrDefault(x => x.Name == "InputCommand`1");
		Console.WriteLine("== dump InputCommand`1 (" + TN(icg) + ") ==");
		if (icg != null) DumpType(icg, true);
		// IInputScheduler
		var iis = AllTypes.FirstOrDefault(x => x.Name == "IInputScheduler");
		Console.WriteLine("== dump IInputScheduler (" + TN(iis) + ") ==");
		if (iis != null) DumpType(iis, true);
	}

	static void InvokeTest() {
		string bp = File.ReadAllText("/home/hatch/workspace/coi-blueprint-pack/concrete-v1.txt").Trim();
		Console.WriteLine("blueprint string length " + bp.Length + " prefix " + bp.Substring(0, Math.Min(12, bp.Length)));
		var bt = AllTypes.FirstOrDefault(x => x.Name == "Blueprint" && TN(x).Contains("Entities"));
		if (bt == null) { Console.WriteLine("Blueprint type NOT FOUND"); return; }
		const BindingFlags F = BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Static | BindingFlags.Instance;
		foreach (var m in bt.GetMethods(F).Where(m => m.Name.Contains("Deserial") || m.Name.Contains("Import") || m.Name.Contains("Parse"))) {
			Console.WriteLine("candidate: " + Vis(m) + " static=" + m.IsStatic + " " + m.Name + "(" +
				string.Join(", ", m.GetParameters().Select(x => TN(x.ParameterType) + " " + x.Name)) + ") -> " + TN(m.ReturnType));
		}
		// Try the named deserializer with the string as sole arg (reflection reaches internals at runtime in full trust).
		var target = bt.GetMethods(F).FirstOrDefault(m => m.Name == "DeserializeForBlueprints"
			&& m.GetParameters().Length >= 1 && m.GetParameters()[0].ParameterType == typeof(string));
		if (target == null) { Console.WriteLine("no DeserializeForBlueprints(string,...) found"); return; }
		Console.WriteLine("invoking " + target.Name + " vis=" + Vis(target) + " params=" + target.GetParameters().Length);
		try {
			var ps = target.GetParameters();
			var callArgs = new object[ps.Length];
			callArgs[0] = bp;
			for (int i = 1; i < ps.Length; i++) callArgs[i] = ps[i].HasDefaultValue ? ps[i].DefaultValue : (ps[i].ParameterType.IsValueType ? Activator.CreateInstance(ps[i].ParameterType) : null);
			object result = target.Invoke(null, callArgs);
			Console.WriteLine("result type: " + (result == null ? "null" : result.GetType().FullName));
			if (result != null) {
				var itemsProp = result.GetType().GetProperty("Items", F);
				if (itemsProp != null) {
					object items = itemsProp.GetValue(result);
					var cnt = items.GetType().GetProperty("Length") ?? items.GetType().GetProperty("Count");
					Console.WriteLine("Items count: " + (cnt != null ? cnt.GetValue(items).ToString() : "?"));
					// first item's Prototype accessor
					var idx = items.GetType().GetMethod("get_Item");
					if (idx != null) {
						object item0 = idx.Invoke(items, new object[] { 0 });
						var protoProp = item0.GetType().GetProperty("Prototype", F);
						Console.WriteLine("item0 Prototype prop: " + (protoProp != null ? Convert.ToString(protoProp.GetValue(item0)) : "n/a"));
					}
				}
			}
		} catch (Exception ex) {
			Console.WriteLine("INVOKE FAILED: " + ex.GetType().Name + ": " + ex.Message);
			if (ex.InnerException != null) Console.WriteLine("  inner: " + ex.InnerException.GetType().Name + ": " + ex.InnerException.Message);
		}
	}
}
