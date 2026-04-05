# Protocols

Protocols are reusable dosing templates for supplements, peptides, medications, or any substance you take on a recurring basis. You define the schedule once, then start a **run** each time you want to execute the protocol. OwnPulse tracks your adherence over time across multiple runs.

## Creating a protocol

A protocol is a template, not an active schedule. To create one, give it a **Name** and a **Duration** in weeks. Use the quick-pick buttons (2W, 4W, 8W, 12W) or choose a custom number of weeks. You can also add an optional description.

Next, add one or more **lines**. Each line represents a single substance in the protocol. For each line you specify:

- **Substance** -- the name of what you are taking
- **Dose** -- the amount per administration (e.g., 250 mcg, 500 mg)
- **Route** -- how it is administered (SubQ, IM, Oral, Topical, Nasal, IV)
- **Timing** -- when to take it (AM, PM, or any time)

### Setting active days

The **sequencer grid** is where you define which days each line is active. The grid shows every day in the protocol's duration, organized by week. Select individual days, or use a **pattern preset** to fill the grid quickly:

- **Daily** -- every day of the protocol
- **Twice a Week** -- two days per week
- **3x per Week** -- three days per week (Mon/Wed/Fri)
- **Every Other Day** -- alternating days
- **Weekdays** -- Monday through Friday

You can apply a preset and then adjust individual days manually. Different lines in the same protocol can have different schedules -- useful for stacking compounds with different dosing frequencies.

### Copy week forward

Each week column in the sequencer grid has a forward arrow button. Clicking it copies that week's dosing pattern to all subsequent weeks in the protocol. This saves time when you want weeks 2 through 8 to repeat the same pattern you set up in week 1.

The grid also supports a day label toggle between numbered days (D1, D2, D3) and weekday names (Mon, Tue, Wed), which can make scheduling easier to visualize.

## Starting a run

A protocol by itself does not track doses -- it is a reusable template. To begin tracking, you start a **run**.

1. Open a protocol and select **Start New Run**.
2. Choose a **Start Date** (defaults to today).
3. Optionally enable notifications (see [Dose reminders](#dose-reminders) below).
4. Select **Start Run**.

A run has a lifecycle with four statuses:

- **Active** -- the run is in progress and tracking doses
- **Paused** -- the run is temporarily suspended (use the **Pause** button)
- **Completed** -- you finished the protocol (use the **Complete** button)
- **Archived** -- the run is stored for historical reference

You can pause and resume a run at any time. You can also start multiple runs of the same protocol -- for example, to repeat a cycle after a rest period.

!!! note
    After creating a protocol, OwnPulse offers to start a run immediately. You can also start one later from the protocol detail page.

## Tracking doses

Once a run is active, the protocol view shows each scheduled dose with its status:

- **Completed** -- you logged the dose
- **Missed** -- the scheduled time passed without a log
- **Skipped** -- you explicitly marked the dose as skipped

To log a dose, open the protocol and tap **Log** next to the scheduled entry. To skip a dose (for example, if you are traveling or feeling unwell), tap **Skip**. Both actions are timestamped.

A progress bar at the top of the protocol shows your overall adherence -- completed doses out of total scheduled doses so far.

## Today's doses

The main Dashboard includes a **Today's Doses** widget that aggregates every dose scheduled for today across all your active runs. Each entry shows the substance, dose, unit, and timing. You can **Log** or **Skip** directly from the dashboard without opening the full protocol view.

The widget shows a **pending count badge** when you have doses waiting to be logged. Once all doses for the day are complete, it displays an "All done" confirmation. Pending doses are visually highlighted so you can quickly see what still needs attention.

## Dose reminders

When starting a run, you can enable push notifications for dose reminders.

1. Check **Enable notifications** in the Start Run dialog.
2. Set one or more notification times (e.g., 08:00 and 20:00 for twice-daily reminders).
3. Optionally enable **Repeat if not logged (every 30 min)** to get follow-up reminders until you log or skip the dose.

Notification times are configured per run, so different protocols can remind you at different times.

!!! note
    On iOS, make sure notifications are enabled in your device settings. See [iOS App -- Notifications](ios-app.md#notifications) for setup details.

## Sharing protocols

You can share a protocol with someone else by generating a **Share Link**. Tap **Share** on any protocol to create a link. The link shows the full protocol configuration -- substance names, doses, routes, timing, and the day-by-day schedule. It does not include your personal adherence data.

Anyone with the link can view the protocol. If they have an OwnPulse account, they can **Copy to My Protocols** to import it as a new protocol on their own account. The imported copy is independent -- changes to the original do not propagate.

## Tips

- **Loading phase then maintenance**: Create a protocol with daily dosing for the first two weeks, then use copy week forward to fill the remaining weeks, and adjust the later weeks to every-other-day.
- **Cycling schedules**: Set a protocol duration that covers one full cycle (e.g., 8 weeks on), then start a new run each time you repeat the cycle.
- **Stacking multiple compounds**: Add multiple lines to a single protocol to keep related substances together. Each line gets its own schedule, so you can dose one substance daily and another MWF within the same protocol.
- **Use timing labels consistently**: Pick a convention like "AM" and "PM" and stick with it across protocols. This makes the Today's Doses widget easier to scan.
