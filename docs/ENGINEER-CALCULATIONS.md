# Calculation reference

This describes the implemented model, including its approximations. Event defaults require engineer confirmation. The software proposes decisions; the engineer checks the actual race rules, driver feedback and pit menu.

## What can train a model

A driving feed must identify the local controlled car and match its telemetry slot. A spectator feed cannot supply authoritative resources or driving inputs. Completed laps must pass the lap-count/timing flags, coverage and timing checks. Pit visits, resource increases, yellows, invalid laps, driver-change fragments and acquisition gaps over one second exclude the affected lap.

The car model uses up to fifteen recent eligible laps with matching track and vehicle, and rain within 0.15 of the current reading. These can span drivers: the model is recent car history, not a claim that a newly arrived driver's pace is already known. Pace is the median lap time. Each resource needs at least three clean consumption observations before its median is used. No usable current resource reading/model means no resource scenario forecast. A forecast covering only one resource explicitly says the other is unavailable.

Fuel consumption is the sampled boundary-to-boundary fall in litres. Virtual energy consumption is the fall in a 0–1 fraction. Replenishment is excluded rather than interpreted as negative consumption. Small boundary sampling offsets and gauge precision are reasons to retain reserves.

Imported references stay in the recording library and can be selected for manual comparison. They do not train consumption or enter automatic technique evidence.

## Range and conservative pit-by lap

For current fuel `F`, fuel use per lap `cF`, virtual energy `E` and energy use `cE`:

```text
fuel range   = F / cF
energy range = E / cE
raw range    = minimum of the available resource ranges
```

Absolute reserves are applied before the lap reserve:

```text
safe fuel range   = max(0, F − fuel reserve) / cF
safe energy range = max(0, E − energy reserve) / cE
pit increment     = max(0, floor(minimum available safe range − reserve laps))
```

The UI adds this increment to the displayed current lap. This is a conservative lap marker; the resource range is measured from the car's current position, and a physical pit-entry decision also needs the approach distance.

For example, 18.5 L at 3.1 L/lap gives 5.97 laps. VE of 31% at 8% per lap gives 3.88 laps, so VE limits the car. With 1 L, 1% VE and one lap reserved, the available safe ranges are 5.65 and 3.75 laps; the conservative increment is two laps.

## Pit opportunities and fills

Set pit-entry distance in event settings. Blank approximates entry at start/finish and says so in the forecast. Distance to entry is the forward remainder around the lap. A car already in the pits has zero approach distance.

“Pit now” uses the next reachable entry. “Extend one lap” skips that opportunity and travels another full lap. Fuel and VE are consumed on the approach; the model cannot refill the car before it gets there. The first approach must preserve the configured absolute and lap reserves, just like later laps. A car already in the pits may refill immediately below reserve. A resource with no usable consumption model is explicitly excluded from the forecast; a zero VE reading on an LMP2 car does not block a fuel-only forecast.

At a service opportunity, target resources cover the remaining fixed forecast distance plus reserves, bounded by the reported fuel capacity and 100% VE:

```text
target fuel = min(capacity, cF × (remaining distance in laps + reserve laps) + fuel reserve)
fuel add    = max(0, target fuel − fuel on arrival)
```

VE uses the equivalent fraction calculation. A resource-only scenario does not invent a stop when neither replenishment nor other work is needed. The first fill belongs to the lowest-time feasible scenario after tyre and current-driver constraints are applied. Infeasible scenarios cannot supply the recommended fill. Unknown resources display unavailable values rather than a zero fill.

## Service time

Fuel and VE service are modelled concurrently:

```text
resource service = max(fuel add / fuel rate, VE add / VE rate)
```

The configured parallel-service choice determines whether tyre/driver work overlaps resource service. Transit loss and configured repair time are additional. Current tyre and handover scenarios test those jobs separately; this is not an optimiser for every combination of crew tasks.

Measure rates, transit loss and work times for the actual car. The optional selected-pit estimate is useful supporting information; it does not automatically replace the engineer's assumptions. Pit-lane resource burn, traffic and future cautions can change the outcome.

Tyre-service scenarios assume a full set at every modelled service stop. A confirmed ledger requires `used + 4 × stops ≤ allocation`; an unknown ledger does not certify that fresh tyres are available. Tyre and repair pace benefits are not invented.

The saving scenario assumes 3% lower fuel/VE use and 1% slower laps. It is a what-if calculation that needs comparison with measured saving laps.

## Timed-race distance

The model estimates when the leader next crosses the line after expiry using reported last-lap pace and position. If those are unavailable, it falls back to the car's clean pace and a start/finish approximation. An optional event extra lap adds another leader lap.

Each scenario compares the **same nominal forecast distance**. Service cost therefore cannot make a scenario look faster by removing race distance. Actual service losses, leader stops, cautions and future pace can change the eventual timed-race lap count. Displayed completion times compare plans; they are not guaranteed wall-clock finish times. Practice and qualifying use a labelled rehearsal over the configured race duration from now.

## Driver constraints

Observed drive time accrues between closely spaced authoritative frames. It excludes unobserved gaps and starts when recording starts. The initial garage period is not a driver stint. A same-driver fuel stop does not reset the continuous driver-stint clock; a driver change does.

Manual signed time corrections affect displayed totals and total-time limits without modifying raw history. They remain saved until edited. Reconcile missing driving and clear obsolete corrections for a new race.

The next-decision checks include travel to pit entry: an extension needs the approach plus one full lap within the current driver's configured budget, and a handover must at least reach the entry before that budget expires. Confirm the additional pit-lane and actual handover timing yourself. Full-race resource forecasts do not optimise driver allocation or establish compliance with official fair-share rules.

Minimum-drive warnings use race-session time. If its remaining-time channel is missing, the configured duration minus elapsed session time is the fallback. An absent clock is not treated as zero remaining time. Configured limits approaching are warnings; a reached observed limit is critical.

## Rivals and rejoin

Relative gaps use reported gaps to the leader plus a lap-deficit approximation based on own pace. Zero is inferred only for the leader. Unknown rival gaps are omitted; incomplete class gaps suppress the rejoin estimate.

A rival behind by `g` seconds with a last lap `d` seconds faster has a constant-pace catch estimate of `g / d` laps. The reciprocal case estimates catching a slower car ahead. These are not predictions of opponent pit intentions or traffic.

Rejoin position adds **transit loss only** to the own-car gap and counts class rivals ahead. Its UI label is deliberate: it does not include every service cost or future rival action.

## Corner comparison and stream review

Lap number, distance and lap-start clock use the scoring frame together so the faster telemetry clock cannot roll the displayed lap time early at start/finish. Time delta is elapsed current-lap time minus reference-lap time interpolated at the same distance. Positive is slower. Sections compare elapsed time, first brake application, full throttle after minimum speed, minimum speed and exit speed. Later brake application alone is not an improvement.

Automatic corner notes need three repeated eligible losses over 0.2 s against a compatible reference. Compatibility checks track, vehicle, compound, rain within 0.1 and known starting fuel within 15 L. Setup and traffic still need human review; the notes ask for inspection rather than assigning a cause.

Retained live analysis traces are approximately 10 Hz at the default capture setting. Braking distances have sampling uncertainty. Raw acquired frames retain the configured capture rate and can be exported separately. Native recordings are resampled for descriptive comparison and carry their own unit/alignment warnings.

The workbench supports a selected recorded lap, a live partial lap with an optional reference, and delayed timeline review. Session changes and clock resets separate traces. Increasing stream delay fetches a wider history window. That delay never delays live strategy. Acknowledgement applies to the current lap and severity, so it does not silently mute a later recurrence or escalation.

See [research and sources](ENGINEER-RESEARCH.md), [architecture](ENGINEER-ARCHITECTURE.md) and [validation evidence](ENGINEER-VALIDATION.md).

## Technique alert freshness

Recorded comparisons remain available for review. Live corner alerts require an authoritative feed received within three seconds and three eligible losses by the current driver in the current observed stint. A handover or return for a later stint requires new repeats. A compatible reference may come from another driver of the same car. Alerts give the measured section loss and minimum/exit-speed differences; they do not assign a driving cause from a single channel.
