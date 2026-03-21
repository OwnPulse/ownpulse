# Manual Data Entry

The Data Entry page is where you log health information by hand. It is organized into tabs, each covering a different type of data. Every data type in OwnPulse supports manual entry -- wearables and integrations are optional.

## Check-ins

Rate five subjective metrics on a scale of 1 to 10:

- **Energy** -- how physically energized you feel
- **Mood** -- your overall emotional state
- **Focus** -- your ability to concentrate and stay on task
- **Recovery** -- how well-rested and physically recovered you feel
- **Libido** -- your level of sexual drive

You can submit one check-in per day. If you submit again on the same day, the new values replace the previous ones. There is no limit on how many days you can backfill.

## Interventions

Log substances, medications, and supplements. Each entry includes a name, dosage, unit, and timestamp. OwnPulse is non-judgmental by design -- it does not validate, filter, or warn on substance names. Every intervention you log is treated as legitimate health data.

## Health records

Enter manual health metrics such as heart rate, blood pressure, body temperature, weight, or blood oxygen. This is useful when you do not have a wearable or want to record a reading from a medical device that does not sync automatically.

## Observations

The most flexible data type. Observations cover anything that does not fit neatly into the other categories:

- **Events** -- things that happened (instant or with a duration)
- **Scales** -- custom numeric ratings with a configurable maximum
- **Symptoms** -- track symptom severity over time
- **Notes** -- freeform text entries
- **Context tags** -- label a day or period (e.g., "travel", "fasting", "menstruation")
- **Environmental** -- temperature, humidity, air quality, or other external readings

### Observation types in detail

Each observation type captures a different kind of information:

- **Event (instant)** -- a timestamped event with optional notes. Example: "Sauna session" with a note like "15 min at 90C."
- **Event (duration)** -- an event with a start and end time. Example: "Meditation" from 7:00 to 7:30 AM.
- **Scale** -- a numeric rating with a configurable maximum. Example: "Pain level" rated 6 out of 10.
- **Symptom** -- a symptom name with severity on a 1-10 scale. Example: "Headache" with severity 4.
- **Note** -- free-form text entry, like a journal. No structured fields, just your words.
- **Context tag** -- a categorical marker for a time period. Examples: "Travel", "Fasting", "Sick day." Context tags help you filter and correlate other data against life circumstances.
- **Environmental** -- a measurement with a unit. Example: "Room temperature 22.5C." Useful for tracking conditions that affect sleep or recovery.

## Lab results

Enter data from blood panels and other laboratory tests. Each result includes the test name, value, unit, and reference range. This gives you a longitudinal view of biomarkers like cholesterol, glucose, thyroid hormones, and vitamin levels alongside your daily data.

!!! tip "Backfilling data"
    All data entry forms accept a custom date and time. You can log past entries at any point -- useful for importing data from paper records or other apps.

## Editing and deleting records

Records can be deleted from the API. Each record type (health records, interventions, observations, lab results) supports deletion by ID. Check-ins use upsert behavior -- submitting a new check-in for the same date replaces the previous values automatically. There is no separate edit action for check-ins; just submit updated scores for the same day.

## Backfilling past data

All data entry forms support selecting past dates and times. You can backfill historical data at any point -- there is no time limit on how far back you can go. This is useful for importing records from paper logs, other apps, or medical documents. Backfilled entries appear on the timeline alongside real-time data with no visual distinction.
