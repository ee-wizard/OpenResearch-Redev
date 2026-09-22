import { queryOptions, useMutation } from "@tanstack/react-query";
import {
  createLibraryFile,
  deleteLibraryFile,
  deleteLibraryItem,
  getLibraryItem,
  listChatAttachments,
  listLibraryItems,
  updateLibraryItem,
  createLibraryItem,
  type CreateLibraryItemBody,
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

/** One file of an item; `path` omitted reads the item's primary file. */
export const getLibraryItemQuery = (
  kind: LibraryKind,
  id: string,
  source: LibrarySource = "team",
  path?: string,
) =>
  queryOptions({
    queryKey: workspaceKey("getLibraryItem", kind, id, source, path ?? null),
    queryFn: ({ signal }) => getLibraryItem(kind, id, source, path, signal),
    staleTime: 30_000,
  });

/** Adding, removing or rewriting a file also changes the folder listing, so
 * every file of the item is invalidated rather than only the edited one. */
const invalidateLibraryItem = (kind: LibraryKind, id: string, source: LibrarySource) =>
  queryClient.invalidateQueries({
    queryKey: workspaceKey("getLibraryItem", kind, id, source),
  });

export function useCreateLibraryItem() {
  return useMutation({
    mutationFn: (req: { kind: LibraryKind; body: CreateLibraryItemBody }) =>
      createLibraryItem(req.kind, req.body),
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
      path?: string;
    }) => updateLibraryItem(req.kind, req.id, req.source, req.content, req.path),
    onSuccess: (_, vars) => {
      void invalidateLibraryItem(vars.kind, vars.id, vars.source);
      void queryClient.invalidateQueries({
        queryKey: workspaceKey("listLibraryItems", vars.kind, vars.source),
      });
    },
  });
}

export function useCreateLibraryFile() {
  return useMutation({
    mutationFn: (req: {
      kind: LibraryKind;
      id: string;
      source: LibrarySource;
      path: string;
      content: string;
    }) => createLibraryFile(req.kind, req.id, req.source, req.path, req.content),
    onSuccess: (_, vars) => {
      void invalidateLibraryItem(vars.kind, vars.id, vars.source);
    },
  });
}

export function useDeleteLibraryFile() {
  return useMutation({
    mutationFn: (req: { kind: LibraryKind; id: string; source: LibrarySource; path: string }) =>
      deleteLibraryFile(req.kind, req.id, req.source, req.path),
    onSuccess: (_, vars) => {
      void invalidateLibraryItem(vars.kind, vars.id, vars.source);
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
