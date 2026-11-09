#!/usr/bin/env python3
"""history_to_states.py — adapt a snapshot's product-history block into a
§4.3.1 state sequence for the plan executors (tools/plan_check.py and
`coi plan-check --state-seq`).

The vanilla /api/v1/products/history series is NOT a state sequence
(GAME-PLAN-FORMAT.md §4.3.1 says so explicitly): it carries per-product
monthly flows + quantity samples for the PINNED products only — no
settlement, research, census, storage or train state. This adapter
builds the most honest sequence that data supports:

  * one state dir per stored history point (8 points), oldest first;
  * stats.json per state holds ONLY the history products, with
    quantity = the series point and produced/consumed truncated to
    [thisMonth, lastMonth] — the windows the payload actually proves.
    Slots thisYear/lastYear/lifetime are ABSENT (short arrays), which
    both engines read as UNEVALUABLE (verified by test, see the
    generated README). stored/storageCapacity are present-but-null
    (the history payload has neither; null reads UNEVALUABLE in both
    engines, while OMITTING the keys breaks the Python engine's
    storage.percent_full cross-reference — also verified by test).
  * game-info.json per state: calendar month derived (see README for
    the alignment evidence), date = last day of the month (day 30),
    tick derived by anchoring the documented 2,880 ticks/day rate to
    the real capture tick; the final month copies live/game-info.json
    verbatim and adds the genuine live bridge-*.json captures (they
    ARE that month's state). Months 1-7 have NO bridge files, so
    every bridge-sourced metric evaluates UNEVALUABLE there — honest
    blindness, not zeroes.
  * products.json + pinned.json are copied from live/ into every
    state (current catalogue + pinned set projected back — caveat in
    the README).

Stdlib only. Read-only over its inputs; writes only under --out-dir.

Usage:
    python3 tools/history_to_states.py [--snapshot PATH]
        [--live-dir live/] [--out-dir history-states/]
"""

import argparse
import json
import pathlib
import shutil
import sys

TICKS_PER_DAY = 2880  # GAME-PLAN-FORMAT.md §4.3.1 / SPEC Addendum XII


def abs_day(y, m, d):
    """Mirror of the engines' _abs_day / abs_day (12 x 30-day months)."""
    return ((y - 1) * 12 + (m - 1)) * 30 + (d - 1)


def parse_date(s):
    y, m, d = (int(p) for p in str(s).split("-"))
    return y, m, d


def add_months(y, m, n):
    idx = (y * 12 + (m - 1)) + n
    return idx // 12, idx % 12 + 1


def load_json(path):
    return json.loads(pathlib.Path(path).read_text())


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    root = pathlib.Path(__file__).resolve().parent.parent
    ap.add_argument("--snapshot", default=str(
        root / "snapshots" / "5583213724730880438-388-2-27-2787522.json"))
    ap.add_argument("--live-dir", default=str(root / "live"))
    ap.add_argument("--out-dir", default=str(root / "history-states"))
    ap.add_argument("--cross-check", default=None,
                    help="hist-keys.json probe file (default: <live-dir>/hist-keys.json if present)")
    args = ap.parse_args()

    snap_path = pathlib.Path(args.snapshot)
    live = pathlib.Path(args.live_dir)
    out = pathlib.Path(args.out_dir)
    cross_path = pathlib.Path(args.cross_check) if args.cross_check \
        else live / "hist-keys.json"

    snap = load_json(snap_path)
    hist = snap["history"]
    game = snap["game"]
    live_game = load_json(live / "game-info.json")
    live_stats = load_json(live / "stats.json")["products"]

    # ---------------------------------------------------------- premises
    errors = []
    if hist.get("resolution") != "month":
        errors.append(f"history resolution is {hist.get('resolution')!r}, not 'month'")
    if game.get("gameId") != live_game.get("gameId"):
        errors.append("snapshot gameId != live game-info gameId")
    if hist.get("date") != game.get("date"):
        errors.append("history date != snapshot game date")
    series = hist["products"]
    lengths = {len(v[k]) for v in series.values() for k in ("produced", "consumed", "quantity")}
    if len(lengths) != 1:
        errors.append(f"history series lengths differ across products/series: {lengths}")
    n = lengths.pop() if lengths else 0

    sy, sm, _ = parse_date(hist["start"])
    gy, gm, gd = parse_date(game["date"])
    months = [add_months(sy, sm, i) for i in range(n)]
    if months[-1] != (gy, gm):
        errors.append(f"last derived month {months[-1]} != game date month {(gy, gm)}")

    # Decisive alignment identities vs the live stats capture (same
    # moment as the history fetch): last point = thisMonth, second
    # last = lastMonth, last two summed = thisYear, last quantity =
    # live quantity. Counted per product; any miss is a hard error.
    ident = {"thisMonth": 0, "lastMonth": 0, "thisYear": 0, "quantity": 0}
    for pid, s in series.items():
        ls = live_stats.get(pid)
        if ls is None:
            errors.append(f"{pid} missing from live stats.json")
            continue
        if s["produced"][-1] == ls["produced"][0] and s["consumed"][-1] == ls["consumed"][0]:
            ident["thisMonth"] += 1
        if s["produced"][-2] == ls["produced"][1] and s["consumed"][-2] == ls["consumed"][1]:
            ident["lastMonth"] += 1
        if (s["produced"][-2] + s["produced"][-1] == ls["produced"][2]
                and s["consumed"][-2] + s["consumed"][-1] == ls["consumed"][2]):
            ident["thisYear"] += 1
        if s["quantity"][-1] == ls["quantity"]:
            ident["quantity"] += 1
    total = len(series)
    for k, v in ident.items():
        if v != total:
            errors.append(f"alignment identity {k} holds for {v}/{total} products")

    # Cross-check probe (hist-keys): its series must equal the tail of
    # the snapshot's, and its start month must be the snapshot's start
    # month + the length difference.
    cross = None
    if cross_path.exists():
        hk = load_json(cross_path)
        hy, hm, _ = parse_date(hk["start"])
        tail_ok, tail_series = 0, 0
        for pid, hv in hk["products"].items():
            sv = series.get(pid)
            if sv is None:
                errors.append(f"cross-check product {pid} absent from snapshot history")
                continue
            for key in ("produced", "consumed", "quantity"):
                tail_series += 1
                if sv[key][len(sv[key]) - len(hv[key]):] == hv[key]:
                    tail_ok += 1
        cross = {"file": str(cross_path), "start": hk["start"],
                 "points": len(next(iter(hk["products"].values()))["quantity"]),
                 "tail_ok": tail_ok, "tail_series": tail_series,
                 "start_month": (hy, hm)}
        if tail_ok != tail_series:
            errors.append(f"cross-check tail equality holds for {tail_ok}/{tail_series} series")
        expect_start = add_months(sy, sm, n - cross["points"])
        if (hy, hm) != expect_start:
            errors.append(f"cross-check start month {(hy, hm)} != expected {expect_start}")

    if errors:
        for e in errors:
            print(f"PREMISE FAILURE: {e}", file=sys.stderr)
        print("refusing to build states on a broken alignment premise",
              file=sys.stderr)
        return 1

    # Conservation diagnostic (reported, not gated): how well quantity
    # deltas match flow nets under same-index vs previous-index
    # readings, over the finished-month transitions.
    prev_exact = same_exact = trans = 0
    prev_abs = same_abs = 0
    for s in series.values():
        q, p, c = s["quantity"], s["produced"], s["consumed"]
        for i in range(1, n - 1):
            trans += 1
            dq = q[i] - q[i - 1]
            r_prev = dq - (p[i - 1] - c[i - 1])
            r_same = dq - (p[i] - c[i])
            prev_exact += r_prev == 0
            same_exact += r_same == 0
            prev_abs += abs(r_prev)
            same_abs += abs(r_same)

    # ---------------------------------------------------------- build
    real_tick = int(game["tick"])
    real_abs = abs_day(gy, gm, gd)
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)

    catalogue_src = live / "products.json"
    pinned_src = live / "pinned.json"
    bridge_names = ["bridge-status.json", "bridge-research.json",
                    "bridge-census.json", "bridge-storages.json",
                    "bridge-trains.json"]

    state_rows = []
    for i, (my, mm) in enumerate(months):
        last = i == n - 1
        dname = f"month-{i + 1:02d}-{my}-{mm:02d}"
        d = out / dname
        d.mkdir()
        if last:
            date = game["date"]
            tick = real_tick
            gi = dict(live_game)  # verbatim copy of the real capture
        else:
            date = f"{my}-{mm}-30"
            tick = real_tick - (real_abs - abs_day(my, mm, 30)) * TICKS_PER_DAY
            gi = {"date": date, "tick": tick,
                  "isPaused": live_game["isPaused"], "speed": live_game["speed"],
                  "gameId": live_game["gameId"], "sessionId": live_game["sessionId"]}
        (d / "game-info.json").write_text(json.dumps(gi, indent=1) + "\n")

        products = {}
        for pid in sorted(series):
            s = series[pid]
            if i == 0:
                prod = [s["produced"][0]]
                cons = [s["consumed"][0]]
            else:
                prod = [s["produced"][i], s["produced"][i - 1]]
                cons = [s["consumed"][i], s["consumed"][i - 1]]
            products[pid] = {"quantity": s["quantity"][i],
                             "stored": None, "storageCapacity": None,
                             "produced": prod, "consumed": cons}
        stats = {"date": date, "tick": tick, "products": products,
                 "unknownIds": []}
        (d / "stats.json").write_text(json.dumps(stats, indent=1) + "\n")

        shutil.copy(catalogue_src, d / "products.json")
        shutil.copy(pinned_src, d / "pinned.json")
        if last:
            for b in bridge_names:
                shutil.copy(live / b, d / b)
        state_rows.append((dname, date, tick, my, mm, last))

    # ---------------------------------------------------------- README
    rows_md = "\n".join(
        f"| {i + 1} | `{dn}/` | {my}-{mm} | {dt} | {tk} |"
        f" {'genuine live capture (verbatim)' if last else 'derived'} |"
        for i, (dn, dt, tk, my, mm, last) in enumerate(state_rows))
    cross_md = ""
    if cross:
        cross_md = (
            f"   Cross-check probe `{cross['file']}` (start {cross['start']}, "
            f"{cross['points']} points): for every shared product and series, "
            f"the snapshot series' trailing {cross['points']} points equal the "
            f"probe's series exactly ({cross['tail_ok']}/{cross['tail_series']} "
            f"series). The probe's start month is exactly the snapshot's start "
            f"month + {n - cross['points']}, i.e. the snapshot holds one more, "
            f"earlier month — the offset-by-one is a window difference, not a "
            f"data disagreement.\n")
    readme = f"""# history-states — product-history adapted into a §4.3.1 state sequence

GENERATED by `tools/history_to_states.py` — do not edit by hand; re-run
the tool. DRY-RUN INPUT ONLY: nothing here was captured from the game at
the stated month-ends except the final state (see below).

## Sources

- History payload: `{snap_path}` — its `history` block is the game's
  `/api/v1/products/history` response (resolution `month`, count 8) for
  the {total} pinned products, fetched at in-game {game['date']} (tick
  {real_tick}), `history.start` = {hist['start']}.
- Catalogue / pinned / final-month game-info + bridge files:
  `{live}/` captures of the same moment ({live_game['date']}).
- Cross-check probe: `live/hist-keys.json` (see alignment evidence).

## Month alignment — decision and evidence

DECISION: stored point *i* (0-based) is calendar month
**{months[0][0]}-{months[0][1]:02d} + i months**, i.e. the eight states are

| # | directory | calendar month | state date | tick | provenance |
|---|-----------|----------------|------------|------|------------|
{rows_md}

The final point is the month in progress ({gy}-{gm:02d}); all earlier
points are finished months. Evidence, strongest first:

1. **Live-stats identities (decisive).** The live `stats.json` capture
   was taken at the same moment as the history fetch. For **{total}/{total}
   products, on both produced and consumed**: history[-1] ==
   stats[thisMonth], history[-2] == stats[lastMonth], and
   history[-2] + history[-1] == stats[thisYear]. The stats windows are
   defined relative to the capture date {game['date']}, so history[-2]
   and history[-1] are pinned to {months[-2][0]}-{months[-2][1]:02d} and
   {gy}-{gm:02d} respectively — no other consecutive-month assignment
   satisfies all three identities.
2. **Start fields.** Counting back at monthly resolution from point 7
   lands point 0 on {months[0][0]}-{months[0][1]:02d} — exactly the month
   of the payload's own `history.start` ({hist['start']}).
{cross_md}3. **Fetch normalisation.** `fetch_live` (coi/src/main.rs) documents
   the raw endpoint returning count+1 points (leading point = state at
   the window start) and drains to the trailing 8 before storing. The
   stored series therefore begins with its window's first *month*, not
   with the pre-window state point — consistent with identities (1)
   pinning the tail, and with (2).
4. **Quantity tail.** history quantity[-1] == live stats quantity for
   {ident['quantity']}/{total} products: the last quantity point is the
   capture-moment stock.

Quantity boundary caveat (bounded, disclosed): for finished months the
payload does not say whether a quantity point is its month's opening or
closing stock, and the flow series cannot settle it — over the {trans}
finished-month transitions across all products, quantity deltas match
the same-month flow net exactly in {same_exact}/{trans} cases (mean
|residual| {same_abs / max(trans, 1):.1f}) and the previous-month net in
{prev_exact}/{trans} cases (mean |residual| {prev_abs / max(trans, 1):.1f});
other quantity channels (trade, construction, in-transit stock) move
quantity without appearing in produced/consumed. This adapter labels
each quantity point with the same month as its flows and dates the
state at that month's last day; any boundary skew is at most one
month's net flow and is NOT corrected by inventing numbers.

## Per-state construction

- `game-info.json` — months 1-{n - 1}: DERIVED. `date` = last day of the
  month (day 30; CoI months have 30 days). `tick` = derived by
  anchoring the documented rate of {TICKS_PER_DAY} ticks/day
  (GAME-PLAN-FORMAT.md §4.3.1) to the real capture: tick(state) =
  {real_tick} − (A({game['date']}) − A(state date)) × {TICKS_PER_DAY},
  where A is the engines' absolute-day index ((Y−1)×12 + (M−1))×30 +
  (D−1). Consecutive states are exactly 30 days / {30 * TICKS_PER_DAY}
  ticks apart, except the last interval ({months[-2][0]}-{months[-2][1]:02d}-30 →
  {game['date']}, {real_abs - abs_day(months[-2][0], months[-2][1], 30)} days),
  which is the real capture date. NOTE: the API tick's epoch is not
  the calendar epoch — the real tick {real_tick} cannot be reproduced
  from the date alone under any day-rate anchored at year 1 — so
  derived ticks are spacing-faithful, not epoch-faithful. Both engines
  order and window states by DATE only; tick is report metadata.
  `gameId`/`sessionId`/`isPaused`/`speed` are copied from the real
  capture (the sequence contract requires one shared gameId; pause /
  speed at past month-ends are unknowable and are labelled derived
  here, never asserted as history). Month {n}: verbatim copy of
  `live/game-info.json`.
- `stats.json` — ONLY the {total} history products. `quantity` = the
  series point. `produced`/`consumed` = SHORT arrays holding exactly
  the windows the payload proves for that state: [0] = that month's
  flow, [1] = the previous month's flow. Month 1 has NO [1] (the
  window starts at {hist['start']}; the month before it is not in any
  capture) — its array has length 1. Slots [2..4]
  (thisYear/lastYear/lifetime) are NOT derivable from an 8-month
  window and are left absent: both engines read a missing flow index
  as UNEVALUABLE (verified by direct test on both engines before this
  encoding was chosen — see NIGHT-LOG round-8 chunk 4). No zeroes are
  fabricated for them. `stored`/`storageCapacity` are present with
  value `null`: the history payload carries neither; null reads
  UNEVALUABLE for ordering predicates in both engines, whereas
  OMITTING the keys makes the Python engine's `storage.percent_full`
  cross-reference raise KeyError and blinds that metric even where
  genuine bridge storage rows exist (also verified by test). The
  wrapper's `date`/`tick` mirror the state's game-info (the engines
  read only `products`).
- `products.json` + `pinned.json` — verbatim copies of the live
  captures in EVERY state. CAVEAT: the catalogue and pinned set are
  the {game['date']} ones projected back over the whole window;
  `product.unlocked` / `product.pinned` predicates therefore read
  today's catalogue at every month. (The pinned set is exactly the
  history product set, so pinned membership is at least
  self-consistent.) Their internal date/tick fields remain the
  capture's — the engines do not read them.
- `bridge-*.json` — ABSENT in months 1-{n - 1} by design: no bridge
  capture exists for those months, so settlement / research /
  census / storage / trains metrics evaluate UNEVALUABLE there.
  Month {n} includes the five genuine live bridge captures
  (status, research, census, storages, trains) — they ARE this
  month's current state. Even there, `research.completed` stays
  UNEVALUABLE: the capture predates CoiBridge v0.6.2 and has no
  `researched` list. No power capture exists in any state.

## Known blind spots (summary)

- Bridge-only metrics: UNEVALUABLE for prefixes ending in months
  1-{n - 1}; evaluable only at the final prefix (latest state = month {n}).
- Non-pinned products (e.g. Product_Fertilizer, Product_IronOre,
  Product_HouseholdGoods): absent from every stats.json →
  UNEVALUABLE at every prefix.
- Flow windows thisYear / lastYear / lifetime: UNEVALUABLE at every
  prefix (short arrays). product.stored / product.storage_capacity:
  UNEVALUABLE at every prefix, including month {n} — the genuine
  final-month values exist only in `live/stats.json`, which this
  adapter deliberately does not substitute into the sequence (uniform
  history-derived construction; see the chunk report).
- `for:` durations can only be proven where every windowed snapshot
  evaluates the inner predicate: any window touching a blind month
  reads UNEVALUABLE, by the §4.3.1 rule — never TRUE by default.
"""
    (out / "README.md").write_text(readme)

    print(f"built {n} states under {out}")
    for dn, dt, tk, my, mm, last in state_rows:
        print(f"  {dn}  date {dt}  tick {tk}" + ("  [genuine final capture]" if last else ""))
    print(f"alignment identities vs live stats: {ident} over {total} products")
    if cross:
        print(f"cross-check tail equality: {cross['tail_ok']}/{cross['tail_series']} series")
    return 0


if __name__ == "__main__":
    sys.exit(main())
