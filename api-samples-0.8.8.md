# CoI web API — live samples from Philip's run (captured 2026-11-02)

Source: demo dashboard paste from Philip's PC, game v0.8.8, Dragon Isle
("Expanding") run, game **paused**, speed 3x. Verbatim samples; long
responses shortened in the dashboard itself with "... N more lines".

## Capture context

- `GET /api/v1/server-info` → `{ "date": "388-1-14", "tick": 2786677, "port": 3073, "otherOrigins": "none" }`
- gameId `5583213724730880438` (stable for this playthrough)
- sessionId `1413194885145907065` (changes per start/load)
- In-game date: **year 388, month 1, day 14** — the save-reader's LOCKED
  Calendar item, served by the vanilla API.

## game-info (verbatim)

```json
{
  "date": "388-1-14",
  "tick": 2786677,
  "isPaused": true,
  "speed": 3,
  "gameId": "5583213724730880438",
  "sessionId": "1413194885145907065"
}
```

Note: no population field — population remains a bridge-mod item.

## products/stats shape (verbatim, 2 of ~40 pinned entries)

Flows are `[thisMonth, lastMonth, thisYear, lastYear, lifetime]`.

```json
"Product_Coal": {
  "quantity": 20145, "stored": 15047, "storageCapacity": 31300,
  "produced": [0, 0, 0, 2600, 770561],
  "consumed": [95, 195, 95, 2358, 748715]
},
"Product_Diesel": {
  "quantity": 6093, "stored": 5327, "storageCapacity": 7800,
  "produced": [60, 96, 60, 1022, 423445], ...
```

`quantity` = total in world; `stored` = in storages; capacity is storage
capacity for that product.

## Pinned products table at capture (dashboard rendering)

| Product | Quantity | Stored / capacity | Prod. last month | Cons. last month |
|---|---|---|---|---|
| Coal | 20,145 | 15,047 / 31,300 | 0 | 195 |
| Iron | 65 | 55 / 200 | 0 | 0 |
| Potato | 856 | 295 / 6,168 | 81 | 33 |
| Diesel | 6,093 | 5,327 / 7,800 | 96 | 140 |
| Concrete slab | 8,012 | 7,221 / 7,524 | 0 | 0 |
| Construction Parts II | 760 | 507 / 507 | 0 | 0 |
| Steel | 1,150 | 312 / 5,700 | 0 | 36 |
| Construction Parts III | 89 | 89 / 500 | 0 | 0 |
| Compost | 1,019 | 680 / 2,000 | 3 | 12 |
| Construction Parts | 763 | 741 / 741 | 0 | 0 |
| Crude oil | 11,421 | 9,239 / 9,000 | 822 | 160 |
| Corn | 2,629 | 2,109 / 6,729 | 92 | 23 |
| Electronics | 8,102 | 7,013 / 8,200 | 84 | 20 |
| Fuel gas | 6,332 | 4,112 / 8,086 | 228 | 244 |
| Glass | 5,561 | 4,861 / 5,713 | 0 | 4 |
| Filter media | 1,840 | 1,453 / 2,014 | 0 | 4 |
| Limestone | 11,869 | 10,186 / 10,906 | 99 | 12 |
| Rail Parts | 1,411 | 1,342 / 1,342 | 0 | 0 |
| Tofu | 400 | 0 / 6,000 | 0 | 0 |
| Sulfur | 12,140 | 11,568 / 20,000 | 23 | 9 |
| Water | 17,067 | 14,471 / 16,418 | 909 | 604 |
| Wood | 5,186 | 4,289 / 5,400 | 0 | 0 |
| Wheat | 5 | 0 / 3,000 | 0 | 0 |
| Vegetables | 814 | 334 / 6,000 | 0 | 0 |
| Tree sapling | 84 | 0 / 200 | 0 | 0 |
| Soybean | 28 | 17 / 3,000 | 0 | 0 |
| Sand | 6,184 | 5,151 / 5,774 | 0 | 12 |
| Salt | 4,433 | 3,915 / 4,013 | 0 | 0 |
| Rubber | 6,736 | 6,123 / 7,000 | 24 | 14 |
| Rock | 17,489 | 12,883 / 15,492 | 407 | 186 |
| Naphtha | 862 | 254 / 1,006 | 184 | 168 |
| Hydrogen | 27,580 | 18,287 / 36,100 | 332 | 54 |
| Dirt | 11,117 | 9,405 / 10,380 | 21 | 0 |
| Bread | 997 | 733 / 10,233 | 24 | 16 |
| Animal feed | 1,333 | 853 / 4,000 | 0 | 0 |
| Biomass | 920 | 680 / 1,200 | 0 | 0 |

## products/sources shape (verbatim, Coal, lastMonth)

```json
"Product_Coal": {
  "produced": { "total": 0, "rows": [] },
  "consumed": { "total": 195, "rows": [
    { "type": "recipe", "quantity": 165, "recipe": "SteamGenerationCoal", "machine": "BoilerCoal" },
    { "type": "recipe", "quantity": 30, "recipe": "CopperSmeltingT2", "machine": "SmeltingFurnaceT2" }
  ] }
},
"names": {
  "SteamGenerationCoal": "Water + Coal → Steam (high) + Exhaust",
  "BoilerCoal": "Boiler",
  "CopperSmeltingT2": "Copper ore crushed + Sand + Coal → Molten copper + Slag + Exhaust"
}
```

Rows can also be `type: "entity"` with an `entity` id (per the route doc).

## products/history shape

`{ date, tick, resolution, start, products: { <id>: { <series>: [oldest … current] } }, unknownIds }`
— arrays hold finished periods from `start`, oldest first, then the period
in progress as the last point. Coal monthly consumed over 25 months ran
~158–243/month — a long, stable series the save RE never reached.

## Errors (verbatim shapes)

- Invalid value → `400 { "error": "invalidParameter", "parameter": "resolution", "message": "Invalid value 'week'. Allowed values: day, month, year, quarterCentury." }`
- Unknown parameter → `400 { "error": "invalidParameter", "parameter": "serie", "message": "Unknown parameter 'serie'. Allowed parameters: ids, pinned, series." }`
- Unknown product ids are NOT errors: they land in `unknownIds`.
- Errors under /api/ are JSON; errors before the path is read are plain
  text — check Content-Type before parsing.

## Dashboard mechanics worth copying

- Polls game-info every few seconds; a **new sessionId drops all cached
  state**; catalog refetched per session and monthly (unlocks change it).
- Speed buttons POST input commands to `/command`; the response arrives
  once the game processed the command and reports success.
- Icons: `GET /api/v1/icons/Product_Acid?v=<sessionId>`.
