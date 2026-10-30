import { invoke } from "@tauri-apps/api/core";
import type {
  Application,
  ApplicationUsage,
  AvailableUpdate,
  Category,
  CategoryInput,
  DateRange,
  Exclusion,
  ExclusionKind,
  ManualSessionInput,
  ManualTimerInput,
  Project,
  ProjectInput,
  RankedItem,
  Report,
  Rule,
  RuleInput,
  RulePreview,
  Session,
  SessionPatch,
  SessionQuery,
  Settings,
  TrackerStatus,
  UpdateInfo,
} from "../types";

export const api = {
  getSettings: () => invoke<Settings>("get_settings"),
  updateSettings: (settings: Settings) => invoke<Settings>("update_settings", { settings }),
  completeOnboarding: (enableTracking: boolean) =>
    invoke<Settings>("complete_onboarding", { enableTracking }),

  trackerStatus: () => invoke<TrackerStatus | null>("tracker_status"),
  pauseTracking: (minutes: number | null) => invoke<void>("pause_tracking", { minutes }),
  resumeTracking: () => invoke<void>("resume_tracking"),
  startManualTimer: (input: ManualTimerInput) => invoke<void>("start_manual_timer", { input }),
  stopManualTimer: () => invoke<void>("stop_manual_timer"),
  installShellIntegration: () => invoke<void>("install_shell_integration"),
  removeShellIntegration: () => invoke<void>("remove_shell_integration"),

  getReport: (range: DateRange, focusCategories?: number[]) =>
    invoke<Report>("get_report", { ...range, focusCategories: focusCategories ?? null }),

  listSessions: (query: SessionQuery) => invoke<Session[]>("list_sessions", { query }),
  updateSession: (id: number, patch: SessionPatch) =>
    invoke<Session>("update_session", { id, patch }),
  deleteSession: (id: number) => invoke<void>("delete_session", { id }),
  createManualSession: (input: ManualSessionInput) =>
    invoke<Session>("create_manual_session", { input }),

  listApplications: () => invoke<Application[]>("list_applications"),
  listApplicationUsage: (range: DateRange) =>
    invoke<ApplicationUsage[]>("list_application_usage", { ...range }),
  updateApplication: (
    id: number,
    patch: { displayName: string | null; defaultProjectId: number | null; defaultCategoryId: number | null },
  ) => invoke<void>("update_application", { id, patch }),
  deleteApplication: (id: number) => invoke<void>("delete_application", { id }),
  applicationIcon: (id: number) => invoke<string | null>("application_icon", { id }),

  listProjects: (includeArchived: boolean) => invoke<Project[]>("list_projects", { includeArchived }),
  createProject: (input: ProjectInput) => invoke<Project>("create_project", { input }),
  updateProject: (id: number, input: ProjectInput) => invoke<Project>("update_project", { id, input }),
  archiveProject: (id: number, archived: boolean) => invoke<void>("archive_project", { id, archived }),
  deleteProject: (id: number) => invoke<void>("delete_project", { id }),
  projectBreakdown: (id: number, range: DateRange) =>
    invoke<RankedItem[]>("project_breakdown", { id, ...range }),

  listCategories: () => invoke<Category[]>("list_categories"),
  createCategory: (input: CategoryInput) => invoke<Category>("create_category", { input }),
  updateCategory: (id: number, input: CategoryInput) =>
    invoke<Category>("update_category", { id, input }),
  deleteCategory: (id: number) => invoke<void>("delete_category", { id }),

  listRules: () => invoke<Rule[]>("list_rules"),
  createRule: (input: RuleInput) => invoke<Rule>("create_rule", { input }),
  updateRule: (id: number, input: RuleInput) => invoke<Rule>("update_rule", { id, input }),
  deleteRule: (id: number) => invoke<void>("delete_rule", { id }),
  reorderRules: (ids: number[]) => invoke<void>("reorder_rules", { ids }),
  previewRules: (range: DateRange) => invoke<RulePreview>("preview_rules", { ...range }),
  applyRules: (range: DateRange) => invoke<RulePreview>("apply_rules", { ...range }),

  listExclusions: () => invoke<Exclusion[]>("list_exclusions"),
  addExclusion: (kind: ExclusionKind, value: string) =>
    invoke<void>("add_exclusion", { kind, value }),
  removeExclusion: (id: number) => invoke<void>("remove_exclusion", { id }),

  exportData: (format: "json" | "csv") => invoke<string | null>("export_data", { format }),
  backupDatabase: () => invoke<string | null>("backup_database"),
  restoreDatabase: () => invoke<string | null>("restore_database"),
  deleteAllActivity: () => invoke<number>("delete_all_activity"),

  updateInfo: () => invoke<UpdateInfo>("update_info"),
  checkForUpdate: () => invoke<AvailableUpdate | null>("check_for_update"),
  knownUpdate: () => invoke<AvailableUpdate | null>("known_update"),
  downloadUpdate: () => invoke<void>("download_update"),
  installUpdate: () => invoke<void>("install_update"),
  restartApp: () => invoke<void>("restart_app"),
  quitApp: () => invoke<void>("quit_app"),
};

export function errorMessage(error: unknown): string {
  return typeof error === "string" ? error : error instanceof Error ? error.message : "Something went wrong";
}
