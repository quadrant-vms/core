{{/*
Expand the name of the chart.
*/}}
{{- define "quadrant-vms.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "quadrant-vms.fullname" -}}
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
Create chart name and version as used by the chart label.
*/}}
{{- define "quadrant-vms.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "quadrant-vms.labels" -}}
helm.sh/chart: {{ include "quadrant-vms.chart" . }}
{{ include "quadrant-vms.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/part-of: quadrant-vms
{{- end }}

{{/*
Selector labels
*/}}
{{- define "quadrant-vms.selectorLabels" -}}
app.kubernetes.io/name: {{ include "quadrant-vms.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "quadrant-vms.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "quadrant-vms.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Database URL
*/}}
{{- define "quadrant-vms.databaseUrl" -}}
postgresql://{{ .Values.infrastructure.postgres.env.POSTGRES_USER }}:{{ .Values.infrastructure.postgres.env.POSTGRES_PASSWORD }}@postgres:5432/{{ .Values.infrastructure.postgres.env.POSTGRES_DB }}
{{- end }}
