import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  EnvironmentLibrary,
  EnvironmentReply,
  EnvironmentRequest,
} from "../environmentTypes";

export const environmentDesktop = {
  command: (
    projectId: string,
    request: EnvironmentRequest,
    packageId?: string,
  ) =>
    invoke<EnvironmentReply>("environment_command", {
      projectId,
      packageId,
      request,
    }),
  library: (
    request:
      { operation: "load" } | { operation: "create_project"; name: string },
  ) => invoke<EnvironmentLibrary>("environment_library", { request }),
  choosePNG: () =>
    open({
      directory: false,
      multiple: false,
      title: "Import environment PNG · original bytes preserved",
      filters: [{ name: "PNG image", extensions: ["png"] }],
    }),
};
