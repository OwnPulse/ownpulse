{{/*
Expand the name of the chart.
*/}}
{{- define "observability.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "observability.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Chart label value.
*/}}
{{- define "observability.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels for a component.
Usage: {{ include "observability.labels" (dict "component" "grafana" "context" $) }}
*/}}
{{- define "observability.labels" -}}
helm.sh/chart: {{ include "observability.chart" .context }}
app.kubernetes.io/name: {{ .component }}
app.kubernetes.io/instance: {{ .context.Release.Name }}
app.kubernetes.io/version: {{ .context.Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .context.Release.Service }}
app.kubernetes.io/part-of: {{ include "observability.name" .context }}
{{- end }}

{{/*
Selector labels for a component.
Usage: {{ include "observability.selectorLabels" (dict "component" "grafana" "context" $) }}
*/}}
{{- define "observability.selectorLabels" -}}
app.kubernetes.io/name: {{ .component }}
app.kubernetes.io/instance: {{ .context.Release.Name }}
{{- end }}

{{/*
Component fullname helper.
Usage: {{ include "observability.componentName" (dict "component" "grafana" "context" $) }}
*/}}
{{- define "observability.componentName" -}}
{{- printf "%s-%s" (include "observability.fullname" .context) .component | trunc 63 | trimSuffix "-" }}
{{- end }}
