# Protocols

Protocols are dosing schedules for supplements, peptides, medications, or any substance you take on a recurring basis. Instead of logging each dose manually every day, you define the schedule once and OwnPulse tracks your adherence over time.

## Creating a protocol

To create a protocol, give it a **Name**, a **Duration** (in days or weeks), and a **Start Date**. This sets the overall timeframe -- for example, "BPC-157 — 4 weeks" starting March 1.

Next, add one or more **lines**. Each line represents a single substance in the protocol. For each line you specify:

- **Substance** -- the name of what you are taking
- **Dose** -- the amount per administration (e.g., 250 mcg, 500 mg)
- **Route** -- how it is administered (oral, sublingual, subcutaneous, topical, etc.)
- **Timing** -- when to take it (morning, evening, with meals, before bed)

### Setting active days

The **sequencer grid** is where you define which days each line is active. The grid shows every day in the protocol's duration. Select individual days, or use a **pattern preset** to fill the grid quickly:

- **Daily** -- every day of the protocol
- **MWF** -- Monday, Wednesday, Friday
- **Every Other Day** -- alternating days
- **Weekdays** -- Monday through Friday

You can apply a preset and then adjust individual days manually. Different lines in the same protocol can have different schedules -- useful for stacking compounds with different dosing frequencies.

## Tracking doses

Once a protocol is active, the protocol view shows each scheduled dose with its status:

- **Completed** -- you logged the dose
- **Missed** -- the scheduled time passed without a log
- **Skipped** -- you explicitly marked the dose as skipped

To log a dose, open the protocol and tap **Log Dose** next to the scheduled entry. To skip a dose (for example, if you are traveling or feeling unwell), tap **Skip**. Both actions are timestamped.

A progress bar at the top of the protocol shows your overall adherence -- completed doses out of total scheduled doses so far.

## Today's doses

The dashboard includes a **Today's Doses** widget that shows every dose scheduled for today across all your active protocols. Each entry shows the substance, dose, and timing. You can **Log** or **Skip** directly from the dashboard without opening the full protocol view.

## Sharing protocols

You can share a protocol with someone else by generating a **Share Link**. Tap **Share** on any protocol to create a link. The link shows the full protocol configuration -- substance names, doses, routes, timing, and the day-by-day schedule. It does not include your personal adherence data.

Anyone with the link can view the protocol. If they have an OwnPulse account, they can **Copy to My Protocols** to import it as a new protocol on their own account. The imported copy is independent -- changes to the original do not propagate.

## Tips

- **Loading phase then maintenance**: Create a protocol with daily dosing for the first two weeks, then edit the sequencer grid to switch to every-other-day for the remaining weeks.
- **Cycling schedules**: Set a protocol duration that covers one full cycle (e.g., 8 weeks on), then create a second protocol for the off period if you want to track time between cycles.
- **Stacking multiple compounds**: Add multiple lines to a single protocol to keep related substances together. Each line gets its own schedule, so you can dose one substance daily and another MWF within the same protocol.
- **Use timing labels consistently**: Pick a convention like "AM" and "PM" or "morning" and "evening" and stick with it across protocols. This makes the Today's Doses widget easier to scan.
