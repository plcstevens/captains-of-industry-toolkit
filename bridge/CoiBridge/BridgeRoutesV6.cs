using System;
using System.Text;
using Mafi;
using Mafi.Collections;
using Mafi.Core.Entities;
using Mafi.Core.Entities.Priorities;
using Mafi.Core.Factory.ElectricPower;
using Mafi.Core.Stats;
using Mafi.Core.Population.Edicts;
using Mafi.Core.Trains;
using Mafi.Core.WebApi;
using Mafi.Core.World;
using Mafi.Core.World.Contracts;
using Mafi.Core.World.Loans;
using Mafi.Core.World.QuickTrade;

namespace CoiBridge;

/// <summary>
/// v0.6 read routes: the state the vanilla web API does not expose at
/// all (verified 2026-11-02: api-routes-0.8.8.json contains zero
/// routes for trade, contracts, loans or edicts).
///
/// Surfaces, all read from the shipped assemblies the same evening:
///   * ContractsManager — [GlobalDependency], ActiveContracts,
///     GetUnlockedContractsButNotActive, IsEstablished, GetToBuy;
///     ContractProto carries ProductToBuy / ProductToPayWith /
///     QuantityToPayWith / UpointsPerMonth / UpointsToEstablish /
///     MinReputationRequired.
///   * LoansManager — CreditScore, InterestRate, Fee,
///     MaxActiveLoans, ActiveLoans, GetAllProductsToBorrow;
///     ActiveLoan carries Product, LeftToPay, AnnualPayment,
///     InterestRate, YearsLeft/YearsPaid, NextPaymentDate.
///   * EdictsManager (Mafi.Core.Population.Edicts) —
///     [GlobalDependency(AsEverything)]; AllEdicts of Edict with
///     Prototype, IsEnabled, IsActive, CanBeEnabled(). The manager
///     also processes ToggleEdictEnabledCmd — an InputCommand that
///     the web API's /commands inventory does NOT list, which is why
///     edicts had no command surface until now. (Read route only in
///     v0.6; actuation stays with the game's own UI until the
///     companion grows an edicts action through the manager.)
///   * TrainLine detail — Id, OwnerNetwork, Schedule items.
///   * WorldMapManager (v0.6.1, recon round 2) —
///     [GlobalDependency(AsEverything)]; AllQuickTrades is an
///     IIndexable&lt;QuickTradeProvider&gt; over the manager's provider
///     list (the same collection its QuickTradeCmd processor matches
///     by Prototype.Id). QuickTradeProvider carries the Prototype
///     (QuickTradePairProto), UpointsCost (Upoints → Fix32 Value),
///     CurrentStep, IsSoldOut, HasPriceIncreased and the pure
///     GetProductToBuy() / GetProductToPayWith() getters returning
///     ProductQuantity {Product, Quantity}.
///   * ElectricityManager (v0.6.2, game-plan gap G2) —
///     [GlobalDependency(AsEverything)] in
///     Mafi.Core.Factory.ElectricPower; the this-tick aggregates
///     GeneratedThisTick / ConsumedThisTick / DemandedThisTick /
///     SurplusDemandedThisTick / GenerationCapacityThisTick /
///     MaxGenerationCapacity / WastedElectricityThisTick plus
///     GeneratorsCount / ConsumersCount. Electricity.Value is exact
///     kW (OneKw = new Electricity(1), verified in the shipped
///     struct), so the route reports raw .Value as kW with no
///     scaling. There is NO battery / electricity-storage aggregate
///     on the manager (recon 2026-11-03: no Battery / Accumulator /
///     ElectricityStorage surface exists; thermal storage holds a
///     product, visible via product stats instead). v0.6.3 extends
///     the power route with the manager's ElectricityAvgStats
///     averages (ProductionStats / ConsumptionStats /
///     GenerationCapacityStats, typed LastDay / LastMonth / LastYear
///     props, raw .Value = kW): the decoding follows the
///     recon-round-4 layout (monthly scalar = rollup average of
///     daily samples; daily = last-tick sample) and is UNVERIFIED —
///     no live cross-check has been possible, so the route flags
///     the block with "averagesUnverified": true.
///   * Custom priorities (v0.6.3, recon round 4 (c)) —
///     IEntityWithCustomPriority (Mafi.Core.Entities.Priorities) has
///     NO id enumeration surface and GetCustomPriority ASSERTS on an
///     unknown id; the route therefore probes the 7 known ids ONLY
///     through IsCustomPriorityVisible (which never asserts) and
///     calls GetCustomPriority ONLY for ids that returned visible.
///     The 7 ids are the per-class hardcoded universe: ImportPrio,
///     ExportPrio, FuelImportPrio, FuelExportPrio, CargoExportPrio,
///     ShipRepairImportPrio, WorldCargoImportPrio.
///
/// This class is a SEPARATE global dependency from BridgeRoutes and
/// is resolved in its own try/catch in CoiBridgeMod.Initialize, so a
/// resolution failure here can never take the v0.5 routes down.
///
/// Same hard rules as BridgeRoutes: read-only, Guard-wrapped.
/// </summary>
[GlobalDependency(RegistrationMode.AsEverything)]
public sealed class BridgeRoutesV6 {

	private readonly ContractsManager m_contracts;
	private readonly LoansManager m_loans;
	private readonly EdictsManager m_edicts;
	private readonly TrainLinesManager m_trainLines;
	private readonly WorldMapManager m_worldMap;
	private readonly ElectricityManager m_power;
	private readonly IEntitiesManager m_entities;

	/// <summary>The full custom-priority id universe (recon round 4
	/// (c)): 7 per-class hardcoded strings across the six
	/// IEntityWithCustomPriority implementers. There is no runtime
	/// enumeration surface; this table is compile-time knowledge and
	/// only a fresh assembly recon will see a future id.</summary>
	private static readonly string[] s_customPriorityIds = {
		"ImportPrio",
		"ExportPrio",
		"FuelImportPrio",
		"FuelExportPrio",
		"CargoExportPrio",
		"ShipRepairImportPrio",
		"WorldCargoImportPrio",
	};

	public BridgeRoutesV6(ContractsManager contracts, LoansManager loans,
			EdictsManager edicts, TrainLinesManager trainLines,
			WorldMapManager worldMap, ElectricityManager power,
			IEntitiesManager entities) {
		m_contracts = contracts;
		m_loans = loans;
		m_edicts = edicts;
		m_trainLines = trainLines;
		m_worldMap = worldMap;
		m_power = power;
		m_entities = entities;
		Log.Info("CoiBridge: BridgeRoutesV6 constructed (contracts/loans/edicts/quick-trades/power/priorities injected)");
	}

	/// <summary>Contracts: established contracts with live trade
	/// estimates, plus unlocked-but-not-established offers.</summary>
	[WebRoute("/coi/v1/contracts", WebRouteStage.Background, "GET", "Contracts: active contracts and available offers.")]
	public WebRouteResponse Contracts() {
		return BridgeRoutes.Guard("/coi/v1/contracts", () => {
			var sb = new StringBuilder();
			sb.Append("{\"profitMultiplierRaw\":").Append(m_contracts.ProfitMultiplier.RawValue)
				.Append(",\"active\":[");
			bool first = true;
			foreach (ContractProto c in m_contracts.ActiveContracts) {
				if (!first) sb.Append(',');
				first = false;
				AppendContract(sb, c, true);
			}
			sb.Append("],\"available\":[");
			var avail = new Lyst<ContractProto>();
			m_contracts.GetUnlockedContractsButNotActive(avail);
			first = true;
			foreach (ContractProto c in avail) {
				if (!first) sb.Append(',');
				first = false;
				AppendContract(sb, c, false);
			}
			sb.Append("]}");
			return sb.ToString();
		});
	}

	private void AppendContract(StringBuilder sb, ContractProto c, bool active) {
		sb.Append("{\"id\":\"").Append(BridgeRoutes.JsonEscape(c.Id.Value))
			.Append("\",\"productToBuy\":\"").Append(BridgeRoutes.JsonEscape(c.ProductToBuy.Id.Value))
			.Append("\",\"productToPayWith\":\"").Append(BridgeRoutes.JsonEscape(c.ProductToPayWith.Id.Value))
			.Append("\",\"quantityToPayWith\":").Append(c.QuantityToPayWith.Value)
			.Append(",\"upointsPerMonthRaw\":").Append(c.UpointsPerMonth.Value.RawValue)
			.Append(",\"upointsToEstablishRaw\":").Append(c.UpointsToEstablish.Value.RawValue)
			.Append(",\"minReputationRequired\":").Append(c.MinReputationRequired);
		if (active) {
			sb.Append(",\"toBuy\":").Append(m_contracts.GetToBuy(c).Value);
		}
		sb.Append('}');
	}

	/// <summary>Loans: credit terms and every active loan.</summary>
	[WebRoute("/coi/v1/loans", WebRouteStage.Background, "GET", "Loans: credit score, terms, active loans, borrowable products.")]
	public WebRouteResponse Loans() {
		return BridgeRoutes.Guard("/coi/v1/loans", () => {
			var sb = new StringBuilder();
			sb.Append("{\"creditScoreRaw\":").Append(m_loans.CreditScore.RawValue)
				.Append(",\"interestRateRaw\":").Append(m_loans.InterestRate.RawValue)
				.Append(",\"feeRaw\":").Append(m_loans.Fee.RawValue)
				.Append(",\"maxActiveLoans\":").Append(m_loans.MaxActiveLoans)
				.Append(",\"activeLoans\":[");
			bool first = true;
			foreach (ActiveLoan l in m_loans.ActiveLoans) {
				if (!first) sb.Append(',');
				first = false;
				sb.Append("{\"id\":").Append(l.Id)
					.Append(",\"product\":\"").Append(BridgeRoutes.JsonEscape(l.Product.Id.Value))
					.Append("\",\"leftToPay\":").Append(l.LeftToPay.Value)
					.Append(",\"annualPayment\":").Append(l.AnnualPayment.Value)
					.Append(",\"interestRateRaw\":").Append(l.InterestRate.RawValue)
					.Append(",\"yearsLeft\":").Append(l.YearsLeft)
					.Append(",\"yearsPaid\":").Append(l.YearsPaid)
					.Append(",\"nextPaymentDate\":\"").Append(BridgeRoutes.JsonEscape(l.NextPaymentDate.ToString()))
					.Append("\",\"interestPaid\":").Append(l.InterestPaid.Value)
					.Append(",\"startingDebt\":").Append(l.StartingDept.Value)
					.Append('}');
			}
			sb.Append("],\"borrowableProducts\":[");
			first = true;
			foreach (var p in m_loans.GetAllProductsToBorrow()) {
				if (!first) sb.Append(',');
				first = false;
				sb.Append('"').Append(BridgeRoutes.JsonEscape(p.Id.Value)).Append('"');
			}
			sb.Append("]}");
			return sb.ToString();
		});
	}

	/// <summary>Edicts: every edict with its enabled/active state and
	/// the game's own reason when it cannot be enabled or is not
	/// currently active.</summary>
	[WebRoute("/coi/v1/edicts", WebRouteStage.Background, "GET", "Edicts: enabled/active state for every edict.")]
	public WebRouteResponse Edicts() {
		return BridgeRoutes.Guard("/coi/v1/edicts", () => {
			var sb = new StringBuilder();
			sb.Append("{\"edicts\":[");
			bool first = true;
			foreach (Edict e in m_edicts.AllEdicts) {
				if (!first) sb.Append(',');
				first = false;
				Edict.EdictEnableCheckResult check = e.CanBeEnabled();
				sb.Append("{\"id\":\"").Append(BridgeRoutes.JsonEscape(e.Prototype.Id.Value))
					.Append("\",\"isEnabled\":").Append(e.IsEnabled ? "true" : "false")
					.Append(",\"isActive\":").Append(e.IsActive ? "true" : "false")
					.Append(",\"canBeEnabled\":").Append(check.CanBeEnabled ? "true" : "false")
					.Append(",\"reason\":\"").Append(BridgeRoutes.JsonEscape(check.Explanation ?? ""))
					.Append("\",\"inactiveReason\":\"").Append(BridgeRoutes.JsonEscape(e.LastReasonForNotBeingActive ?? ""))
					.Append("\"}");
			}
			sb.Append("]}");
			return sb.ToString();
		});
	}

	/// <summary>Quick trades: every provider on the world map with its
	/// live buy/pay quantities (volume-multiplier applied by the game's
	/// own getters), current unity cost, and step/sold-out state.
	/// Backing: WorldMapManager.AllQuickTrades (recon round 2). Pure
	/// reads only — CanAfford (out-param in this build) is deliberately
	/// not called.</summary>
	[WebRoute("/coi/v1/quick-trades", WebRouteStage.Background, "GET", "Quick trades: providers, live quantities, unity cost, step state.")]
	public WebRouteResponse QuickTrades() {
		return BridgeRoutes.Guard("/coi/v1/quick-trades", () => {
			var sb = new StringBuilder();
			sb.Append("{\"trades\":[");
			bool first = true;
			foreach (QuickTradeProvider p in m_worldMap.AllQuickTrades) {
				if (!first) sb.Append(',');
				first = false;
				var buy = p.GetProductToBuy();
				var pay = p.GetProductToPayWith();
				sb.Append("{\"id\":\"").Append(BridgeRoutes.JsonEscape(p.Prototype.Id.Value))
					.Append("\",\"productToBuy\":{\"product\":\"").Append(BridgeRoutes.JsonEscape(buy.Product.Id.Value))
					.Append("\",\"quantity\":").Append(buy.Quantity.Value)
					.Append("},\"productToPayWith\":{\"product\":\"").Append(BridgeRoutes.JsonEscape(pay.Product.Id.Value))
					.Append("\",\"quantity\":").Append(pay.Quantity.Value)
					.Append("},\"upointsCostRaw\":").Append(p.UpointsCost.Value.RawValue)
					.Append(",\"currentStep\":").Append(p.CurrentStep)
					.Append(",\"maxSteps\":").Append(p.Prototype.MaxSteps)
					.Append(",\"isSoldOut\":").Append(p.IsSoldOut ? "true" : "false")
					.Append(",\"hasPriceIncreased\":").Append(p.HasPriceIncreased ? "true" : "false")
					.Append('}');
			}
			sb.Append("]}");
			return sb.ToString();
		});
	}

	/// <summary>Train lines in detail: ids, owning network, schedule
	/// items (station entity ids where the item exposes one).</summary>
	[WebRoute("/coi/v1/trains-detail", WebRouteStage.Background, "GET", "Train lines in detail: ids, networks, schedules.")]
	public WebRouteResponse TrainsDetail() {
		return BridgeRoutes.Guard("/coi/v1/trains-detail", () => {
			var sb = new StringBuilder();
			sb.Append("{\"lines\":[");
			bool first = true;
			foreach (TrainLine line in m_trainLines.Lines) {
				if (!first) sb.Append(',');
				first = false;
				sb.Append("{\"id\":").Append(line.Id.Value)
					.Append(",\"name\":\"").Append(BridgeRoutes.JsonEscape(line.Name.ToString()))
					.Append("\",\"trains\":").Append(line.TrainsCount)
					.Append(",\"stations\":").Append(line.StationsCount)
					.Append(",\"network\":");
				if (line.OwnerNetwork.HasValue) {
					sb.Append("{\"id\":").Append(line.OwnerNetwork.Value.Id.Value)
						.Append(",\"name\":\"").Append(BridgeRoutes.JsonEscape(line.OwnerNetwork.Value.Name.ToString()))
						.Append("\"}");
				} else {
					sb.Append("null");
				}
				sb.Append(",\"schedule\":[");
				bool f2 = true;
				foreach (ITrainScheduleItem item in line.Schedule) {
					if (!f2) sb.Append(',');
					f2 = false;
					sb.Append("{\"type\":\"").Append(BridgeRoutes.JsonEscape(item.GetType().Name))
						.Append("\"}");
				}
				sb.Append("]}");
			}
			sb.Append("]}");
			return sb.ToString();
		});
	}

	/// <summary>Electric power: this-tick generation, demand,
	/// consumption and balance in kW, straight from
	/// ElectricityManager's exact aggregates (Electricity.Value is
	/// kW — no scaling). balanceKw is generated − consumed. No
	/// battery/storage field: the manager exposes no electricity
	/// storage aggregate (see the class comment).
	///
	/// v0.6.3 adds "averages" from the three ElectricityAvgStats
	/// objects' typed props (LastDay / LastMonth / LastYear, raw
	/// .Value = kW, unscaled). The decoding follows the
	/// recon-round-4 layout — monthly scalar = rollup average of
	/// daily samples; daily = last-tick sample — and is UNVERIFIED:
	/// no live cross-check against the in-game Statistics window
	/// has been possible, hence the sibling
	/// "averagesUnverified": true flag.</summary>
	[WebRoute("/coi/v1/power", WebRouteStage.Background, "GET", "Electric power: generation, demand, consumption, balance in kW.")]
	public WebRouteResponse Power() {
		return BridgeRoutes.Guard("/coi/v1/power", () => {
			int generated = m_power.GeneratedThisTick.Value;
			int consumed = m_power.ConsumedThisTick.Value;
			var sb = new StringBuilder();
			sb.Append("{\"generatedKw\":").Append(generated)
				.Append(",\"consumedKw\":").Append(consumed)
				.Append(",\"demandedKw\":").Append(m_power.DemandedThisTick.Value)
				.Append(",\"surplusDemandedKw\":").Append(m_power.SurplusDemandedThisTick.Value)
				.Append(",\"generationCapacityKw\":").Append(m_power.GenerationCapacityThisTick.Value)
				.Append(",\"maxGenerationCapacityKw\":").Append(m_power.MaxGenerationCapacity.Value)
				.Append(",\"wastedKw\":").Append(m_power.WastedElectricityThisTick.Value)
				.Append(",\"balanceKw\":").Append(generated - consumed)
				.Append(",\"generators\":").Append(m_power.GeneratorsCount)
				.Append(",\"consumers\":").Append(m_power.ConsumersCount)
				.Append(",\"averages\":{");
			AppendAverages(sb, "production", m_power.ProductionStats);
			sb.Append(',');
			AppendAverages(sb, "consumption", m_power.ConsumptionStats);
			sb.Append(',');
			AppendAverages(sb, "generationCapacity", m_power.GenerationCapacityStats);
			sb.Append("},\"averagesUnverified\":true}");
			return sb.ToString();
		});
	}

	private static void AppendAverages(StringBuilder sb, string name, ElectricityAvgStats stats) {
		sb.Append('"').Append(name).Append("\":{\"lastDayKw\":").Append(stats.LastDay.Value)
			.Append(",\"lastMonthKw\":").Append(stats.LastMonth.Value)
			.Append(",\"lastYearKw\":").Append(stats.LastYear.Value)
			.Append('}');
	}

	/// <summary>Custom priorities: every entity implementing
	/// IEntityWithCustomPriority with at least one visible id, and
	/// only its visible ids. SAFETY (recon round 4 (c)): the 7 known
	/// ids are probed ONLY through IsCustomPriorityVisible — it
	/// never asserts and returns false for an inapplicable id — and
	/// GetCustomPriority is called ONLY for an id that returned
	/// visible=true (the game's own UI discipline). Get/Set are
	/// NEVER called with an unprobed id; Set is never called at all
	/// (read-only bridge).</summary>
	[WebRoute("/coi/v1/priorities", WebRouteStage.Background, "GET", "Custom priorities: per-entity visible custom priority ids and values.")]
	public WebRouteResponse Priorities() {
		return BridgeRoutes.Guard("/coi/v1/priorities", () => {
			var sb = new StringBuilder();
			sb.Append("{\"ids\":[");
			for (int i = 0; i < s_customPriorityIds.Length; i++) {
				if (i > 0) sb.Append(',');
				sb.Append('"').Append(BridgeRoutes.JsonEscape(s_customPriorityIds[i])).Append('"');
			}
			sb.Append("],\"entities\":[");
			bool firstEntity = true;
			foreach (IEntity e in m_entities.Entities) {
				if (e.IsDestroyed) continue;
				if (!(e is IEntityWithCustomPriority cp)) continue;
				var prios = new StringBuilder();
				bool firstPrio = true;
				foreach (string id in s_customPriorityIds) {
					// Visibility gate first — never assert, never guess.
					if (!cp.IsCustomPriorityVisible(id)) continue;
					if (!firstPrio) prios.Append(',');
					firstPrio = false;
					AppendPriority(prios, id, cp.GetCustomPriority(id));
				}
				if (firstPrio) continue; // no visible id on this entity
				if (!firstEntity) sb.Append(',');
				firstEntity = false;
				sb.Append("{\"id\":").Append(e.Id.Value)
					.Append(",\"proto\":\"").Append(BridgeRoutes.JsonEscape(e.Prototype.Id.Value))
					.Append("\",\"priorities\":{").Append(prios.ToString()).Append("}}");
			}
			sb.Append("]}");
			return sb.ToString();
		});
	}

	private static void AppendPriority(StringBuilder sb, string id, int value) {
		sb.Append('"').Append(BridgeRoutes.JsonEscape(id)).Append("\":").Append(value);
	}
}
