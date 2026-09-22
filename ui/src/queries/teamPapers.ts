import { queryOptions, useMutation } from "@tanstack/react-query";
import * as api from "../api";
import { queryClient, workspaceKey } from "./client";

export const listTeamPapersQuery = (projectId: string) =>
  queryOptions({
    queryKey: workspaceKey("listTeamPapers", projectId),
    queryFn: ({ signal }) => api.listTeamPapers(projectId, signal),
    staleTime: 30_000,
  });

export const getTeamPaperTextQuery = (projectId: string, paperId: string) =>
  queryOptions({
    queryKey: workspaceKey("getTeamPaperText", projectId, paperId),
    queryFn: ({ signal }) => api.getTeamPaperText(projectId, paperId, signal),
    staleTime: 30_000,
  });

function invalidateTeamPapers(projectId: string, paperId?: string) {
  void queryClient.invalidateQueries({
    queryKey: workspaceKey("listTeamPapers", projectId),
  });
  if (paperId) {
    void queryClient.invalidateQueries({
      queryKey: workspaceKey("getTeamPaperText", projectId, paperId),
    });
  }
}

export function useUploadTeamPaper() {
  return useMutation({
    mutationFn: api.uploadTeamPaper,
    onSuccess: (_, vars) => invalidateTeamPapers(vars.projectId),
  });
}

export function useUpdateTeamPaper() {
  return useMutation({
    mutationFn: api.updateTeamPaper,
    onSuccess: (_, vars) => invalidateTeamPapers(vars.projectId, vars.id),
  });
}

export function useDeleteTeamPaper() {
  return useMutation({
    mutationFn: (req: { projectId: string; id: string }) =>
      api.deleteTeamPaper(req.projectId, req.id),
    onSuccess: (_, vars) => invalidateTeamPapers(vars.projectId, vars.id),
  });
}
