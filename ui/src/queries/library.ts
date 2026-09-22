import { queryOptions, useMutation } from "@tanstack/react-query";
import {
  deleteLibraryItem,
  getLibraryItem,
  listChatAttachments,
  listLibraryItems,
  updateLibraryItem,
  createLibraryItem,
  type LibraryKind,
  type LibrarySource,
} from "../api";
import { queryClient, workspaceKey } from "./client";

export const listLibraryItemsQuery = (kind?: LibraryKind, source: LibrarySource = "team") =>
  queryOptions({
    queryKey: workspaceKey("listLibraryItems", kind ?? null, source),
    queryFn: ({ signal }) => listLibraryItems(kind, source, signal),
    staleTime: 30_000,
  });

export const listChatAttachmentsQuery = () =>
  queryOptions({
    queryKey: workspaceKey("listChatAttachments"),
    queryFn: ({ signal }) => listChatAttachments(signal),
    staleTime: 30_000,
  });

export const getLibraryItemQuery = (
  kind: LibraryKind,
  id: string,
  source: LibrarySource = "team",
) =>
  queryOptions({
    queryKey: workspaceKey("getLibraryItem", kind, id, source),
    queryFn: ({ signal }) => getLibraryItem(kind, id, source, signal),
    staleTime: 30_000,
  });

export function useCreateLibraryItem() {
  return useMutation({
    mutationFn: (req: { kind: LibraryKind; name: string; content: string }) =>
      createLibraryItem(req.kind, req.name, req.content),
    onSuccess: (_, vars) => {
      void queryClient.invalidateQueries({
        queryKey: workspaceKey("listLibraryItems", vars.kind, "team"),
      });
    },
  });
}

export function useUpdateLibraryItem() {
  return useMutation({
    mutationFn: (req: {
      kind: LibraryKind;
      id: string;
      source: LibrarySource;
      content: string;
    }) => updateLibraryItem(req.kind, req.id, req.source, req.content),
    onSuccess: (_, vars) => {
      void queryClient.invalidateQueries({
        queryKey: workspaceKey("getLibraryItem", vars.kind, vars.id, vars.source),
      });
      void queryClient.invalidateQueries({
        queryKey: workspaceKey("listLibraryItems", vars.kind, vars.source),
      });
    },
  });
}

export function useDeleteLibraryItem() {
  return useMutation({
    mutationFn: (req: { kind: LibraryKind; id: string; source: LibrarySource }) =>
      deleteLibraryItem(req.kind, req.id, req.source),
    onSuccess: (_, vars) => {
      void queryClient.invalidateQueries({
        queryKey: workspaceKey("listLibraryItems", vars.kind, vars.source),
      });
    },
  });
}
