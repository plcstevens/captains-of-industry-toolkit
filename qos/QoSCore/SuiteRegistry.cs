using System.Collections.Generic;
using Mafi;

namespace QoSCore;

/// <summary>
/// The suite's phone book. Feature mods call
/// <see cref="Announce"/> from their Initialize; the registry logs
/// the suite composition as it assembles. It deliberately holds no
/// behaviour and no feature depends on it to function — it exists
/// so the suite presents as one thing (one composition line in the
/// log) and so a future core service has a single place that knows
/// which features are loaded.
///
/// Registered both ways, like the shipped PriorityManager
/// controllers: the [GlobalDependency] attribute here and an
/// explicit RegisterDependency call in QoSCoreMod.
/// </summary>
[GlobalDependency(RegistrationMode.AsEverything)]
public sealed class SuiteRegistry {

	private readonly List<string> m_features = new List<string>();

	/// <summary>Announced features, as "QoSFeatureId x.y.z", in announce order.</summary>
	public IReadOnlyList<string> Features {
		get { return m_features; }
	}

	/// <summary>Record a loaded suite member and log the composition so far.</summary>
	public void Announce(string featureId, string version) {
		string entry = featureId + " " + version;
		if (m_features.Contains(entry)) {
			return;
		}
		m_features.Add(entry);
		QoSLog.Info("Core", "suite member loaded: " + entry +
			" (suite now: " + string.Join(", ", m_features) + ")");
	}
}
