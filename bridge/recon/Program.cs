using System;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Collections.Generic;

// Spike-4 recon harness: substring type search + full member dump.
// Usage: dotnet run -- <Substring>          -> list types whose FullName contains it
//        dotnet run -- dump:<FullTypeName>  -> dump one type (all members, declared only, walks base chain)
//        dotnet run -- impl:<Substring>     -> list non-abstract types assignable to the named interface/type
class Program {
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
                try { AllTypes.AddRange(asm.GetTypes()); }
                catch (ReflectionTypeLoadException ex) { AllTypes.AddRange(ex.Types.Where(t => t != null)); }
            } catch (Exception ex) { Console.WriteLine("LOADFAIL " + f + " " + ex.GetType().Name); }
        }
    }
    static string TN(Type t) { return t == null ? "?" : (t.FullName ?? t.Name); }
    static void Dump(Type t) {
        Console.WriteLine("TYPE " + TN(t) + " : " + TN(t.BaseType) + (t.IsEnum ? " [ENUM]" : "") + (t.IsInterface ? " [INTERFACE]" : "") + (t.IsAbstract ? " [abstract]" : ""));
        foreach (var i in t.GetInterfaces()) Console.WriteLine("  iface " + TN(i));
        if (t.IsEnum) { foreach (var n in Enum.GetNames(t)) Console.WriteLine("  enumval " + n); return; }
        const BindingFlags F = BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance | BindingFlags.Static | BindingFlags.DeclaredOnly;
        foreach (var p in t.GetProperties(F)) Console.WriteLine("  prop " + (p.GetMethod != null && p.GetMethod.IsPublic ? "pub " : "nonpub ") + TN(p.PropertyType) + " " + p.Name);
        foreach (var f in t.GetFields(F)) Console.WriteLine("  field " + (f.IsPublic ? "pub " : "nonpub ") + TN(f.FieldType) + " " + f.Name + (f.IsLiteral ? " = " + f.GetRawConstantValue() : ""));
        foreach (var m in t.GetMethods(F).Where(m => !m.IsSpecialName))
            Console.WriteLine("  meth " + (m.IsPublic ? "pub " : "nonpub ") + TN(m.ReturnType) + " " + m.Name + "(" + string.Join(", ", m.GetParameters().Select(x => TN(x.ParameterType) + " " + x.Name)) + ")");
        foreach (var nt in t.GetNestedTypes(BindingFlags.Public | BindingFlags.NonPublic)) Console.WriteLine("  nested " + TN(nt));
    }
    static void Main(string[] args) {
        LoadAll();
        Console.WriteLine("TOTAL TYPES " + AllTypes.Count);
        if (args.Length == 0) { Console.WriteLine("no arg"); return; }
        var a = args[0];
        if (a.StartsWith("dump:")) {
            var name = a.Substring(5);
            var t = AllTypes.FirstOrDefault(x => TN(x) == name);
            if (t == null) { Console.WriteLine("NOT-FOUND " + name); return; }
            var cur = t;
            while (cur != null && cur != typeof(object)) { Dump(cur); cur = cur.BaseType; if (cur != null) Console.WriteLine("-- base --"); }
        } else if (a.StartsWith("impl:")) {
            var name = a.Substring(5);
            var it = AllTypes.FirstOrDefault(x => TN(x) == name);
            if (it == null) { Console.WriteLine("NOT-FOUND " + name); return; }
            foreach (var t in AllTypes.Where(x => it.IsAssignableFrom(x)).OrderBy(x => TN(x)))
                Console.WriteLine("IMPL " + TN(t) + (t.IsAbstract ? " [abstract]" : "") + (t.IsInterface ? " [interface]" : ""));
        } else {
            foreach (var t in AllTypes.Where(x => TN(x).IndexOf(a, StringComparison.OrdinalIgnoreCase) >= 0).OrderBy(x => TN(x)))
                Console.WriteLine("HIT " + TN(t) + (t.IsEnum ? " [ENUM]" : "") + (t.IsInterface ? " [INTERFACE]" : ""));
        }
    }
}
