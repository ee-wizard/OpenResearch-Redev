import { queryOptions, useMutation } from "@tanstack/react-query";
import { getHandbookChapter, listHandbookChapters, saveHandbookChapter } from "../api";
import { queryClient, workspaceKey } from "./client";

export const listHandbookChaptersQuery = () =>
  queryOptions({
    queryKey: workspaceKey("handbookChapters"),
    queryFn: ({ signal }) => listHandbookChapters(signal),
    staleTime: 30_000,
  });

export const getHandbookChapterQuery = (id: string) =>
  queryOptions({
    queryKey: workspaceKey("handbookChapter", id),
    queryFn: ({ signal }) => getHandbookChapter(id, signal),
    staleTime: 30_000,
  });

export function useSaveHandbookChapter() {
  return useMutation({
    mutationFn: (req: { id: string; content: string }) =>
      saveHandbookChapter(req.id, req.content),
    onSuccess: (_, vars) => {
      void queryClient.invalidateQueries({
        queryKey: workspaceKey("handbookChapter", vars.id),
      });
      void queryClient.invalidateQueries({
        queryKey: workspaceKey("handbookChapters"),
      });
    },
  });
}
