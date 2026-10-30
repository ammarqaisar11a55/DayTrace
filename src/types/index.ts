export type SessionType = "active" | "unknown" | "manual" | "idle" | "locked";
export type Theme = "system" | "light" | "dark";
export type UpdateChannel = "stable" | "beta";
export type ExclusionKind = "application" | "path" | "project";
export type TrackingState = "disabled" | "paused" | "tracking" | "idle" | "locked" | "manual";
export type Support = "available" | "limited" | "unavailable";
export type IntegrationState = "notInstalled" | "pendingRestart" | "active";
export type InstallMethod = "nsis" | "appimage" | "deb" | "rpm" | "snap" | "unsupported";

export interface Settings {
  onboardingCompleted: boolean;
  trackingEnabled: boolean;
  trackingConsentAt: number | null;
  pollIntervalSecs: number;
  idleThresholdSecs: number;
  collectWindowTitles: boolean;
  retentionDays: number;
  theme: Theme;
  startOnLogin: boolean;
  runInBackground: boolean;
  startHidden: boolean;
  updateChannel: UpdateChannel;
  updateCheckOnStartup: boolean;
  updateCheckIntervalHours: number;
  lastUpdateCheck: number | null;
}

export interface Capability {
  support: Support;
  detail: string | null;
}

export interface Capabilities {
  platform: string;
  displayServer: string;
  desktop: string;
  backend: string;
  foregroundApp: Capability;
  executablePath: Capability;
  windowTitle: Capability;
  idle: Capability;
  lock: Capability;
  suspend: Capability;
  integration: IntegrationState | null;
}

export interface TrackerStatus {
  state: TrackingState;
  currentApplication: string | null;
  pausedUntil: number | null;
  manualLabel: string | null;
  manualStartedAt: number | null;
  capabilities: Capabilities;
}

export interface Application {
  id: number;
  platformIdentifier: string;
  name: string;
  displayName: string | null;
  executablePath: string | null;
  iconPath: string | null;
  defaultProjectId: number | null;
  defaultCategoryId: number | null;
  createdAt: number;
}

export interface ApplicationUsage extends Application {
  totalMs: number;
  sessionCount: number;
  lastUsed: number | null;
}

export interface Project {
  id: number;
  parentId: number | null;
  name: string;
  description: string;
  color: string;
  status: "active" | "archived";
  createdAt: number;
  updatedAt: number;
}

export interface ProjectInput {
  parentId: number | null;
  name: string;
  description: string;
  color: string;
}

export interface Category {
  id: number;
  name: string;
  color: string;
  defaultProjectId: number | null;
  createdAt: number;
  updatedAt: number;
}

export interface CategoryInput {
  name: string;
  color: string;
  defaultProjectId: number | null;
}

export interface Session {
  id: number;
  applicationId: number | null;
  applicationName: string | null;
  executablePath: string | null;
  projectId: number | null;
  projectName: string | null;
  projectColor: string | null;
  categoryId: number | null;
  categoryName: string | null;
  categoryColor: string | null;
  startTime: number;
  endTime: number;
  durationSeconds: number;
  sessionType: SessionType;
  windowTitle: string | null;
  label: string | null;
  manualOverride: boolean;
  isOpen: boolean;
}

export interface SessionQuery {
  from: string;
  to: string;
  applicationId?: number | null;
  projectId?: number | null;
  categoryId?: number | null;
  limit?: number;
  newestFirst?: boolean;
}

export interface SessionPatch {
  projectId: number | null;
  categoryId: number | null;
  label: string | null;
  startTime?: number;
  endTime?: number;
}

export interface ManualSessionInput {
  startTime: number;
  endTime: number;
  label: string | null;
  projectId: number | null;
  categoryId: number | null;
}

export interface ManualTimerInput {
  label: string | null;
  projectId: number | null;
  categoryId: number | null;
}

export type ConditionField = "application" | "executablePath" | "windowTitle";
export type ConditionOperator = "equals" | "contains" | "startsWith" | "endsWith";

export interface Condition {
  field: ConditionField;
  operator: ConditionOperator;
  value: string;
}

export interface Rule {
  id: number;
  name: string;
  conditions: Condition[];
  targetProjectId: number | null;
  targetCategoryId: number | null;
  priority: number;
  enabled: boolean;
}

export type RuleInput = Omit<Rule, "id" | "priority">;

export interface RuleChange {
  sessionId: number;
  applicationName: string;
  startTime: number;
  durationSeconds: number;
  oldProjectId: number | null;
  newProjectId: number | null;
  oldCategoryId: number | null;
  newCategoryId: number | null;
}

export interface RulePreview {
  examined: number;
  changed: number;
  skippedManual: number;
  samples: RuleChange[];
}

export interface Exclusion {
  id: number;
  kind: ExclusionKind;
  value: string;
  createdAt: number;
}

export interface Totals {
  activeMs: number;
  idleMs: number;
  unknownMs: number;
  manualMs: number;
  totalMs: number;
  untrackedMs: number;
}

export interface RankedItem {
  id: number | null;
  name: string;
  color: string | null;
  ms: number;
  share: number;
}

export interface Bucket {
  key: string;
  activeMs: number;
  idleMs: number;
}

export interface HourCell {
  weekday: number;
  hour: number;
  ms: number;
}

export interface Report {
  from: string;
  to: string;
  totals: Totals;
  applications: RankedItem[];
  projects: RankedItem[];
  categories: RankedItem[];
  hourly: Bucket[];
  daily: Bucket[];
  weekly: Bucket[];
  monthly: Bucket[];
  heatmap: HourCell[];
}

export interface UpdateInfo {
  currentVersion: string;
  channel: UpdateChannel;
  installMethod: InstallMethod;
  canInstall: boolean;
  guidance: string | null;
}

export interface AvailableUpdate {
  version: string;
  notes: string | null;
  date: string | null;
}

export interface UpdateProgress {
  downloaded: number;
  total: number | null;
}

export interface DateRange {
  from: string;
  to: string;
}
