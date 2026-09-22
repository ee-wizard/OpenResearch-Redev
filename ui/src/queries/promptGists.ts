import { queryOptions, useMutation } from "@tanstack/react-query";
import * as api from "../api";
import { queryClient, workspaceKey } from "./client";

export const listPromptGistsQuery = (projectId?: string) =>
  queryOptions({
    queryKey: workspaceKey("listPromptGists", projectId ?? null),
    queryFn: ({ signal }) => api.listPromptGists(projectId, signal),
    staleTime: 30_000,
  });

export function useCreatePromptGist() {
  return useMutation({
    mutationFn: api.createPromptGist,
    onSuccess: (_, vars) => {
      void queryClient.invalidateQueries({
        queryKey: workspaceKey("listPromptGists", vars.projectId ?? null),
      });
    },
  });
}

export function useUpdatePromptGist() {
  return useMutation({
    mutationFn: (req: {
      id: string;
      projectId?: string | null;
      name?: string;
      content?: string;
      description?: string;
      tags?: string[];
    }) => api.updatePromptGist(req.id, req),
    onSuccess: (_, vars) => {
      void queryClient.invalidateQueries({
        queryKey: workspaceKey("listPromptGists", vars.projectId ?? null),
      });
    },
  });
}

export function useDeletePromptGist() {
  return useMutation({
    mutationFn: (req: { id: string; projectId?: string | null }) =>
      api.deletePromptGist(req.id, req.projectId ?? undefined),
    onSuccess: (_, vars) => {
      void queryClient.invalidateQueries({
        queryKey: workspaceKey("listPromptGists", vars.projectId ?? null),
      });
    },
  });
}
