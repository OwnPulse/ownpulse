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

Once a run is active, the protocol view shows a **schedule grid** with one cell per scheduled (substance, day) pair. Each cell has one of four statuses:

- **Completed** -- you logged the dose
- **Skipped** -- you explicitly marked the dose as skipped
- **Missed** -- a scheduled day in the past with no log or skip -- this is derived, not something you set directly
- **Pending** -- a scheduled day today or in the future, not yet logged

Every **Missed** or **Pending** cell is itself a control. Click it to open a small popover with **Log** and **Skip** options:

- **Log** accepts an optional time and notes. If you leave the time blank, OwnPulse uses the line's configured timing (AM/PM/etc).
- **Skip** accepts an optional reason (for example, "traveling"). The reason is stored with the skip -- OwnPulse never judges or filters what you write here.

This is how you **backfill**: if you missed logging a dose two days ago, open the grid and log or skip that day's cell directly -- there's no special "backdate" mode, the same popover handles it.

Already-logged cells (**Completed** or **Skipped**) show an **Undo** action instead, which deletes the dose entry so you can re-log it or leave it unset.

### Adherence

The protocol view shows an adherence summary above the grid, for example "83% adherence · 20 done · 2 skipped · 2 missed". This is computed by the server, over **closed days only** -- scheduled days strictly before today. Today and future days never count toward adherence, even if you've already logged them early. Days inside a paused interval are excluded entirely (pausing a run stops the adherence clock, it does not accrue missed doses).

Skipped doses are excluded from the adherence percentage's denominator -- skipping a dose for a legitimate reason does not count against you. If nothing has closed yet (a run that just started, or every closed day was skipped), the summary reads "No closed days yet" instead of a percentage.

A separate, secondary progress bar shows elapsed time through the run (days passed / total duration) -- this is just a clock, unrelated to adherence.

## Today's doses

The main Dashboard includes a **Today's Doses** widget that aggregates every dose scheduled for today across all your active runs. Each entry shows the substance, dose, unit, and timing. You can **Log** or **Skip** directly from the dashboard without opening the full protocol view.

The widget shows a **pending count badge** when you have doses waiting to be logged. Once all doses for the day are complete, it displays an "All done" confirmation. Pending doses are visually highlighted so you can quickly see what still needs attention.

### Reviewing missed doses

If you have missed doses from earlier days across any of your active runs, the widget shows a **"N missed dose(s) from earlier days -- Review"** toggle below today's list. Expanding it lists each missed dose with its date, protocol, and substance, with the same **Log** / **Skip** actions as the schedule grid -- so you can backfill without leaving the dashboard. The review list is capped at 200 entries; for a complete history on one run, use that protocol's schedule grid.

## Dose reminders

When starting a run, you can enable reminders for its doses.

1. Check **Enable notifications** in the Start Run dialog.
2. Set one or more notification times (e.g., 08:00 and 20:00 for twice-daily reminders).

Notification times are configured per run, so different protocols can remind you at different times.

!!! warning "iOS only, and local to the device"
    Dose reminders are delivered as **local notifications scheduled on your iPhone** -- there is no push-notification backend, and the web app does not send or receive reminders. Enabling notifications on a run only affects the iOS devices where you're signed in; the setting itself is stored on the run so any iOS device can pick it up. See [iOS App -- Notifications](ios-app.md#notifications) for how the iOS app schedules and refreshes them, and for setup details.

!!! note
    "Repeat if not logged" is not implemented. A reminder fires once per configured time and does not follow up if the dose is not logged or skipped.

## Sharing protocols

You can share a protocol with someone else by generating a **Share Link**. Tap **Share** on any protocol to create a link. The link shows the full protocol configuration -- substance names, doses, routes, timing, and the day-by-day schedule. It does not include your personal adherence data.

Anyone with the link can view the protocol. If they have an OwnPulse account, they can **Copy to My Protocols** to import it as a new protocol on their own account. The imported copy is independent -- changes to the original do not propagate.

## Tips

- **Loading phase then maintenance**: Create a protocol with daily dosing for the first two weeks, then use copy week forward to fill the remaining weeks, and adjust the later weeks to every-other-day.
- **Cycling schedules**: Set a protocol duration that covers one full cycle (e.g., 8 weeks on), then start a new run each time you repeat the cycle.
- **Stacking multiple compounds**: Add multiple lines to a single protocol to keep related substances together. Each line gets its own schedule, so you can dose one substance daily and another MWF within the same protocol.
- **Use timing labels consistently**: Pick a convention like "AM" and "PM" and stick with it across protocols. This makes the Today's Doses widget easier to scan.
