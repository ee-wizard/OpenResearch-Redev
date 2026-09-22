import { queryOptions, useMutation } from "@tanstack/react-query";
import {
  createHandbookChapter,
  deleteHandbookChapter,
  getHandbookChapter,
  listHandbookChapters,
  saveHandbookChapter,
} from "../api";
import { queryClient, workspaceKey } from "./client";

/** `projectId === null` lists the team-wide chapters only; a project id lists
 * those plus that project's own. */
export const listHandbookChaptersQuery = (projectId: string | null = null) =>
  queryOptions({
    queryKey: workspaceKey("handbookChapters", projectId),
    queryFn: ({ signal }) => listHandbookChapters(projectId, signal),
    staleTime: 30_000,
  });

export const getHandbookChapterQuery = (id: string, projectId: string | null = null) =>
  queryOptions({
    queryKey: workspaceKey("handbookChapter", projectId, id),
    queryFn: ({ signal }) => getHandbookChapter(id, projectId, signal),
    staleTime: 30_000,
  });

/** The chapter list is the thing that changes shape (create/delete/rename), so
 * every write refreshes it along with the chapter body it touched. */
function invalidateHandbook(projectId: string | null, id?: string) {
  void queryClient.invalidateQueries({ queryKey: workspaceKey("handbookChapters", projectId) });
  if (id) {
    void queryClient.invalidateQueries({ queryKey: workspaceKey("handbookChapter", projectId, id) });
  }
}

export function useSaveHandbookChapter() {
  return useMutation({
    mutationFn: (req: { id: string; content: string; projectId?: string | null }) =>
      saveHandbookChapter(req.id, req.content, req.projectId),
    onSuccess: (_, vars) => invalidateHandbook(vars.projectId ?? null, vars.id),
  });
}

export function useCreateHandbookChapter() {
  return useMutation({
    mutationFn: createHandbookChapter,
    onSuccess: (_, vars) => invalidateHandbook(vars.projectId ?? null),
  });
}

export function useDeleteHandbookChapter() {
  return useMutation({
    mutationFn: (req: { id: string; projectId?: string | null }) =>
      deleteHandbookChapter(req.id, req.projectId),
    onSuccess: (_, vars) => invalidateHandbook(vars.projectId ?? null, vars.id),
  });
}
