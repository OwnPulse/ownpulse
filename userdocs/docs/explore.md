# Explore

The Explore page lets you chart any combination of your health metrics on a single timeline. You can overlay heart rate with mood scores, compare sleep duration against lab results, or visualize any data OwnPulse has collected -- all in one view.

## Adding metrics

To start building a chart, open the metric picker. Your available metrics are organized by source:

- **Health Records** -- heart rate, HRV, resting heart rate, body mass, body fat percentage, body temperature, blood pressure, blood glucose, blood oxygen, respiratory rate, steps, active energy, basal energy, and VO2 max
- **Check-ins** -- energy, mood, focus, recovery, and libido scores
- **Lab Results** -- any lab test you have recorded (e.g., testosterone, TSH, vitamin D)
- **Calendar** -- meeting minutes and meeting count
- **Sleep** -- sleep duration, deep sleep, REM sleep, and sleep score

Select a metric to add it to the chart. You can add up to eight metrics at once.

## Date range

Choose a date range to control which data appears on the chart. Preset options make it quick:

- **7 days** -- the past week
- **30 days** -- the past month
- **90 days** -- the past three months
- **1 year** -- the past year
- **All** -- everything OwnPulse has

You can also set a custom date range by specifying exact start and end dates.

## Resolution

The resolution toggle controls how data points are aggregated:

- **Daily** -- one point per day (average of all readings that day)
- **Weekly** -- one point per week
- **Monthly** -- one point per month

Higher resolutions show more detail. Lower resolutions smooth out noise and reveal trends.

Each data point shows the average value and the number of raw records that were combined to produce it. This helps you understand whether a data point represents a single reading or a full day of measurements.

## Dual Y-axis

When you chart metrics with different units (for example, heart rate in BPM alongside a mood score from 1 to 10), Explore automatically assigns them to separate Y-axes. This keeps the scales readable so that low-range scores are not flattened against high-range measurements.

## Saving charts

Once you have a chart configuration you want to revisit, save it. Saved charts store the full configuration:

1. Set up the metrics, date range, and resolution you want.
2. Save the chart and give it a name (up to 200 characters).
3. Access your saved charts from the charts list at any time.

You can update a saved chart's name or configuration later, or delete it when you no longer need it.

## Loading a saved chart

Open the saved charts list and select a chart to load it. The Explore page restores the exact configuration -- metrics, date range, resolution, and any custom colors you set.

## Real-time updates

When new data arrives -- from a manual entry, a HealthKit sync, or an integration -- the Explore page updates automatically. You do not need to refresh the page. This works through a live connection that listens for data changes on your account.

!!! note
    Real-time updates require an active connection to your OwnPulse instance. If you lose connectivity, the chart will refresh with the latest data when the connection is restored.

## Tips

- Start with a single metric to get oriented, then layer on additional metrics to look for patterns.
- Use the weekly or monthly resolution to spot trends that daily noise might obscure.
- Save charts for metric combinations you check regularly, like morning vitals or a recovery dashboard.
- Lab results are especially useful at monthly resolution -- overlay them against daily metrics to see how biomarker changes correlate with how you feel.
