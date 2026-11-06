using System;
using System.Collections.Generic;
using System.IO;
using System.Reflection;
using QoSBuilder;

// QoSBuilder canonical-vector probe (BUILD-NOTES round 9).
//
// Runs the pinned canonical test vectors (v1 blueprint + the two
// schema-2 designation vectors from DESIGNATION-DESIGN-NOTE §2.3)
// through BuilderJson and prints canonical string + sha256 next to
// the independently computed expectations (v1: the vector pinned in
// coi/src/serve.rs, produced by an independent Python implementation;
// v2: an independent Python implementation of the note's spec).
//
// Modes:
//   (no args)        -> vectors through the BuilderJson SOURCE compiled
//                       into this harness (the exact shipping file)
//   dll <path>       -> vectors through the COMPILED net48 QoSBuilder.dll
//                       itself, by reflection
class CanonProbe {

	const string V1ExpectedSha = "a8b4498e9edb99e4338bbee339a826f51db9f6ad7a0439c72d8e379343069cbf";
	const string AddExpectedSha = "e4166dd35533a12049ee495a5192358f3a262272cc86475c0a79979158ed57f7";
	const string RemoveExpectedSha = "78fb273ccdb9d498cae5e05ecc077d1e18c58dd2c406d59c23d1a8617e70cfa4";

	static int s_failures = 0;

	static void Check(string label, string canonical, string sha, string expectedSha) {
		bool ok = sha == expectedSha;
		if (!ok) s_failures++;
		Console.WriteLine(label + " canonical: " + canonical);
		Console.WriteLine(label + " sha256:    " + sha);
		Console.WriteLine(label + " expected:  " + expectedSha + "  => " + (ok ? "MATCH" : "MISMATCH"));
	}

	static void RunSourceVectors() {
		string v1 = BuilderJson.Canonical(
			"11111111-2222-3333-4444-555555555555", 1,
			"2026-11-05T18:00:00Z", "2026-11-05T18:30:00Z",
			"5583213724730880438", "captain",
			"Test \"A\" \\ Béton", "B1504:H4sIAAAAAAAC/test+string==",
			3167, 762, 4, 90, true, 500);
		Check("V1", v1, BuilderJson.Sha256Hex(v1), V1ExpectedSha);

		var cells = new List<BuilderJson.DesignationCell> {
			new BuilderJson.DesignationCell(100, 200, 12, 12, 11, 12),
			new BuilderJson.DesignationCell(104, 200, 12, 11, 11, 12),
		};
		string add = BuilderJson.CanonicalDesignation(
			"22222222-3333-4444-5555-666666666666", 2, "designations-add",
			"2026-11-06T20:00:00Z", "2026-11-06T20:30:00Z",
			"5583213724730880438", "captain",
			"Test pad \"A\" \\ Béton", "MiningDesignator", cells, null, 200);
		Check("V2-ADD", add, BuilderJson.Sha256Hex(add), AddExpectedSha);

		var origins = new List<BuilderJson.DesignationOrigin> {
			new BuilderJson.DesignationOrigin(100, 200),
			new BuilderJson.DesignationOrigin(108, 204),
		};
		string remove = BuilderJson.CanonicalDesignation(
			"33333333-4444-5555-6666-777777777777", 2, "designations-remove",
			"2026-11-06T20:05:00Z", "2026-11-06T20:35:00Z",
			"5583213724730880438", "captain",
			"Test removal", null, null, origins, 200);
		Check("V2-REMOVE", remove, BuilderJson.Sha256Hex(remove), RemoveExpectedSha);
	}

	static void RunDllVectors(string dllPath) {
		var asm = Assembly.LoadFrom(Path.GetFullPath(dllPath));
		var t = asm.GetType("QoSBuilder.BuilderJson");
		if (t == null) { Console.WriteLine("DLL: QoSBuilder.BuilderJson NOT FOUND"); s_failures++; return; }
		var sha256Hex = t.GetMethod("Sha256Hex");
		Func<string, string> sha = s => (string)sha256Hex.Invoke(null, new object[] { s });

		var canonical = t.GetMethod("Canonical");
		string v1 = (string)canonical.Invoke(null, new object[] {
			"11111111-2222-3333-4444-555555555555", 1L,
			"2026-11-05T18:00:00Z", "2026-11-05T18:30:00Z",
			"5583213724730880438", "captain",
			"Test \"A\" \\ Béton", "B1504:H4sIAAAAAAAC/test+string==",
			3167L, 762L, 4L, 90L, true, 500L });
		Check("DLL V1", v1, sha(v1), V1ExpectedSha);

		var cellType = asm.GetType("QoSBuilder.BuilderJson+DesignationCell");
		var originType = asm.GetType("QoSBuilder.BuilderJson+DesignationOrigin");
		var listCellsType = typeof(List<>).MakeGenericType(cellType);
		var listOriginsType = typeof(List<>).MakeGenericType(originType);
		var cells = Activator.CreateInstance(listCellsType);
		var addCell = listCellsType.GetMethod("Add");
		foreach (var c in new[] { new long[] { 100, 200, 12, 12, 11, 12 }, new long[] { 104, 200, 12, 11, 11, 12 } }) {
			addCell.Invoke(cells, new[] { Activator.CreateInstance(cellType, c[0], c[1], c[2], c[3], c[4], c[5]) });
		}
		var origins = Activator.CreateInstance(listOriginsType);
		var addOrigin = listOriginsType.GetMethod("Add");
		foreach (var o in new[] { new long[] { 100, 200 }, new long[] { 108, 204 } }) {
			addOrigin.Invoke(origins, new[] { Activator.CreateInstance(originType, o[0], o[1]) });
		}

		var canonicalD = t.GetMethod("CanonicalDesignation");
		string add = (string)canonicalD.Invoke(null, new object[] {
			"22222222-3333-4444-5555-666666666666", 2L, "designations-add",
			"2026-11-06T20:00:00Z", "2026-11-06T20:30:00Z",
			"5583213724730880438", "captain",
			"Test pad \"A\" \\ Béton", "MiningDesignator", cells, null, 200L });
		Check("DLL V2-ADD", add, sha(add), AddExpectedSha);

		string remove = (string)canonicalD.Invoke(null, new object[] {
			"33333333-4444-5555-6666-777777777777", 2L, "designations-remove",
			"2026-11-06T20:05:00Z", "2026-11-06T20:35:00Z",
			"5583213724730880438", "captain",
			"Test removal", null, null, origins, 200L });
		Check("DLL V2-REMOVE", remove, sha(remove), RemoveExpectedSha);
	}

	static int Main(string[] args) {
		if (args.Length >= 2 && args[0] == "dll") {
			Console.WriteLine("== vectors through the compiled DLL: " + args[1] + " ==");
			RunDllVectors(args[1]);
		} else {
			Console.WriteLine("== vectors through BuilderJson source compiled into this harness ==");
			RunSourceVectors();
		}
		Console.WriteLine(s_failures == 0 ? "ALL VECTORS MATCH" : ("FAILURES: " + s_failures));
		return s_failures == 0 ? 0 : 1;
	}
}
