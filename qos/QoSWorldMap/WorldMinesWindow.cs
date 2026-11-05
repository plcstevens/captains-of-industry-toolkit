using System;
using System.Collections.Generic;
using Mafi;
using Mafi.Core.Entities;
using Mafi.Core.World;
using Mafi.Core.World.Entities;
using Mafi.Localization;
using Mafi.Unity.Ui;
using Mafi.Unity.UiToolkit;
using Mafi.Unity.UiToolkit.Component;
using Mafi.Unity.UiToolkit.Library;
using QoSCore;

namespace QoSWorldMap;

public enum MineSort {
	Product,
	Level,
	Quantity,
	State,
	Title
}

public enum MineStateFilter {
	All,
	Working,
	Attention,
	Unrepaired
}

/// <summary>
/// The world-mines index window (QOL-MOD-PREP section 2): a
/// PriorityManager-pattern UiToolkit Window over
/// WorldMapManager.Mines — which covers mines AND oil rigs (there
/// is no OilRig type; a rig is a WorldMapMine whose product is
/// crude oil). Rows are read-only views of public game state;
/// clicking one navigates (see MineNavigator). No remote actions
/// live here by design — shifts, repair, upgrade and ship orders
/// belong to the mine's own inspector, which the click opens.
///
/// Live-check seam 3 (fullscreen stacking) applies to this window's
/// ControllerConfig.Window behaviour against the immersive-
/// fullscreen world map; see MineNavigator for the mitigation.
/// </summary>
[GlobalDependency(RegistrationMode.AsEverything)]
public sealed class WorldMinesWindow : Window {

	private const int PollEveryFrames = 10;

	private readonly WorldMapManager m_mapManager;
	private readonly InspectorsManager m_inspectors;
	private readonly QoSWorldMapMod m_mod;

	private readonly Column m_rows = new Column().AlignItemsStretch();
	private readonly Label m_status = new Label();
	private readonly TextField m_filter = new TextField();
	private readonly Dropdown<MineSort> m_sort;
	private readonly Dropdown<MineStateFilter> m_stateFilter;
	private bool m_descending;
	private int m_pollFrames;
	private string m_lastSignature = "";

	public WorldMinesWindow(WorldMapManager mapManager, InspectorsManager inspectors, QoSWorldMapMod mod)
		: base(new LocStrFormatted("World map — mines & rigs")) {
		m_mapManager = mapManager;
		m_inspectors = inspectors;
		m_mod = mod;

		WindowSize(new Px(980), new Px(640));
		MakeMovableAndEnablePositionSaving();

		m_sort = new Dropdown<MineSort>((value, index, inDropdown) => new Label(SortName(value)));
		m_sort.SetOptions(MineSort.Product, MineSort.Level, MineSort.Quantity, MineSort.State, MineSort.Title);
		m_sort.SetValue(MineSort.Product);
		m_sort.OnValueChanged((value, index) => Refresh());

		m_stateFilter = new Dropdown<MineStateFilter>((value, index, inDropdown) => new Label(StateFilterName(value)));
		m_stateFilter.SetOptions(MineStateFilter.All, MineStateFilter.Working, MineStateFilter.Attention, MineStateFilter.Unrepaired);
		m_stateFilter.SetValue(MineStateFilter.All);
		m_stateFilter.OnValueChanged((value, index) => Refresh());

		Panel controls = new Panel(noBolts: true).ReducedPadding().BodyGap(2.pt()).AlignSelfStretch();
		Row filterRow = new Row(2.pt()).AlignItemsCenter();
		m_filter.CharLimit(80).Placeholder(new LocStrFormatted("Filter by name or product")).FlexGrow(1f).MinWidth(260.px())
			.OnValueChanged(_ => Refresh());
		filterRow.Add(m_filter);
		filterRow.Add(new Label(new LocStrFormatted("State")));
		filterRow.Add(m_stateFilter.NoShrink().MinWidth(140.px()));
		filterRow.Add(new Label(new LocStrFormatted("Sort")));
		filterRow.Add(m_sort.NoShrink().MinWidth(130.px()));
		Toggle descendingToggle = new Toggle(standalone: true).Value(false)
			.OnValueChanged(value => { m_descending = value; Refresh(); });
		filterRow.Add(descendingToggle);
		filterRow.Add(new Label(new LocStrFormatted("Descending")));
		filterRow.Add(new ButtonText(Button.General, new LocStrFormatted("Refresh"), Refresh));
		controls.Body.Add(filterRow);
		controls.Body.Add(m_status);
		Body.Add(controls);
		Body.Add(m_rows);
	}

	/// <summary>Rebuild the row list from current game state.</summary>
	public void Refresh() {
		RemoveChildren(m_rows);
		List<WorldMapMine> mines = FilterAndSort();
		m_status.Value(new LocStrFormatted(
			mines.Count + " of " + m_mapManager.Mines.Count + " world-map locations · click a row to centre the map and open its inspector"));
		foreach (WorldMapMine mine in mines) {
			m_rows.Add(BuildRow(mine));
		}
		m_lastSignature = Signature();
	}

	/// <summary>
	/// Cheap change poll, called from the controller's InputUpdate
	/// (the PriorityManager cadence): rebuild only when the visible
	/// state actually moved — a mine depleting, breaking, or a new
	/// location being claimed.
	/// </summary>
	public void PollForChanges() {
		if (++m_pollFrames < PollEveryFrames) {
			return;
		}
		m_pollFrames = 0;
		if (Signature() != m_lastSignature) {
			Refresh();
		}
	}

	private string Signature() {
		// Order-independent aggregate: count + per-mine state,
		// quantity and buffer level. Collisions cost a missed
		// cosmetic refresh, never a wrong action (there are none).
		long acc = m_mapManager.Mines.Count;
		foreach (WorldMapMine mine in m_mapManager.Mines) {
			acc = acc * 31 + (int)mine.CurrentState;
			acc = acc * 31 + (mine.QuantityAvailable.HasValue ? mine.QuantityAvailable.Value.Value : -1);
			acc = acc * 31 + mine.Buffer.Quantity.Value;
			acc = acc * 31 + mine.Level;
		}
		return acc.ToString();
	}

	private List<WorldMapMine> FilterAndSort() {
		string filter = m_filter.GetText().Trim();
		List<WorldMapMine> mines = new List<WorldMapMine>();
		foreach (WorldMapMine mine in m_mapManager.Mines) {
			if (!MatchesStateFilter(mine)) {
				continue;
			}
			if (filter.Length > 0
				&& mine.GetTitle().IndexOf(filter, StringComparison.OrdinalIgnoreCase) < 0
				&& ProductName(mine).IndexOf(filter, StringComparison.OrdinalIgnoreCase) < 0) {
				continue;
			}
			mines.Add(mine);
		}
		// Comparators: primary key, title as the tiebreak, always
		// transitive (an inconsistent comparator makes List.Sort
		// throw). Level/Quantity are "biggest first" by nature; the
		// Descending toggle reverses whatever order was chosen.
		switch (m_sort.SelectedValue) {
			case MineSort.Level:
				mines.Sort((a, b) => FirstNonZero(b.Level.CompareTo(a.Level), TitleCmp(a, b)));
				break;
			case MineSort.Quantity:
				mines.Sort((a, b) => FirstNonZero(QuantityKey(b).CompareTo(QuantityKey(a)), TitleCmp(a, b)));
				break;
			case MineSort.State:
				mines.Sort((a, b) => FirstNonZero(string.Compare(StateText(a), StateText(b), StringComparison.Ordinal), TitleCmp(a, b)));
				break;
			case MineSort.Title:
				mines.Sort((a, b) => TitleCmp(a, b));
				break;
			default: // Product
				mines.Sort((a, b) => FirstNonZero(string.Compare(ProductName(a), ProductName(b), StringComparison.Ordinal), TitleCmp(a, b)));
				break;
		}
		if (m_descending) {
			mines.Reverse();
		}
		return mines;
	}

	private bool MatchesStateFilter(WorldMapMine mine) {
		switch (m_stateFilter.SelectedValue) {
			case MineStateFilter.Working:
				return mine.CurrentState == WorldMapMine.State.Working;
			case MineStateFilter.Attention:
				return mine.IsRepaired && mine.CurrentState != WorldMapMine.State.Working;
			case MineStateFilter.Unrepaired:
				return !mine.IsRepaired;
			default:
				return true;
		}
	}

	private UiComponent BuildRow(WorldMapMine mine) {
		PanelRow row = new PanelRow(2.pt(), noBolts: true).AlignSelfStretch().MarginBottom(2.pt());
		row.OnClick(() => Go(mine));
		row.Body.Add(new Label(new LocStrFormatted(Title(mine))).FlexGrow(1f));
		row.Body.Add(new Label(new LocStrFormatted(ProductName(mine))));
		row.Body.Add(new Label(new LocStrFormatted(StateText(mine))));
		row.Body.Add(new Label(new LocStrFormatted("Lv " + mine.Level + " · " + mine.ProductionStep + "/" + mine.MaxProductionSteps)));
		row.Body.Add(new Label(new LocStrFormatted("Left: " + QuantityText(mine))));
		row.Body.Add(new Label(new LocStrFormatted("Buffer " + mine.Buffer.Quantity.Value + "/" + mine.Buffer.Capacity.Value)));
		row.Body.Add(new ButtonText(Button.General, new LocStrFormatted("Go"), () => Go(mine)));
		return row;
	}

	private void Go(WorldMapMine mine) {
		MineNavigator navigator = m_mod.Navigator;
		if (navigator == null) {
			QoSLog.Warning("WorldMap", "row click before renderer-init completed — ignoring.");
			return;
		}
		QoSLog.Debug("WorldMap", "navigate: " + Title(mine) + " (" + ProductName(mine) + ")");
		navigator.Navigate(mine, m_inspectors);
	}

	private static int FirstNonZero(int first, int second) {
		return first != 0 ? first : second;
	}

	private static int TitleCmp(WorldMapMine a, WorldMapMine b) {
		return string.Compare(Title(a), Title(b), StringComparison.Ordinal);
	}

	private static string Title(WorldMapMine mine) {
		return mine.GetTitle();
	}

	private static string ProductName(WorldMapMine mine) {
		return mine.Product.Id.ToString();
	}

	private static string StateText(WorldMapMine mine) {
		return mine.IsRepaired ? mine.CurrentState.ToString() : "Unrepaired";
	}

	private static string QuantityText(WorldMapMine mine) {
		return mine.QuantityAvailable.HasValue ? mine.QuantityAvailable.Value.Value.ToString() : "∞";
	}

	private static long QuantityKey(WorldMapMine mine) {
		// Unlimited reserves sort as the largest quantity.
		return mine.QuantityAvailable.HasValue ? mine.QuantityAvailable.Value.Value : long.MaxValue;
	}

	private static LocStrFormatted SortName(MineSort sort) {
		switch (sort) {
			case MineSort.Level: return new LocStrFormatted("Level");
			case MineSort.Quantity: return new LocStrFormatted("Quantity left");
			case MineSort.State: return new LocStrFormatted("State");
			case MineSort.Title: return new LocStrFormatted("Name");
			default: return new LocStrFormatted("Product");
		}
	}

	private static LocStrFormatted StateFilterName(MineStateFilter filter) {
		switch (filter) {
			case MineStateFilter.Working: return new LocStrFormatted("Working");
			case MineStateFilter.Attention: return new LocStrFormatted("Needs attention");
			case MineStateFilter.Unrepaired: return new LocStrFormatted("Unrepaired");
			default: return new LocStrFormatted("All");
		}
	}

	private static void RemoveChildren(UiComponent component) {
		while (component.ChildrenCount > 0) {
			component[0].RemoveFromHierarchy();
		}
	}
}
