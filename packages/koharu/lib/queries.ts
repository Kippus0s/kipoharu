'use client'

import {
  MutationCache,
  MutationObserver,
  QueryCache,
  QueryClient,
  type QueryKey,
  queryOptions,
  useIsMutating,
  useMutation,
  useQuery,
} from '@tanstack/react-query'
import type { ParseKeys } from 'i18next'
import { useMemo } from 'react'
import { useDebouncedCallback } from 'use-debounce'

import {
  commands,
  type FontFamily,
  type History,
  type LayerId,
  type Page,
  type PageId,
  type PageSummary,
  type Preferences,
  type ProjectAnswer,
} from '@koharu/ipc/protocol'
import { toast } from '@koharu/ui/components/toast'

import i18n from './i18n'

export const projectKey = ['project'] as const
export const pageKey = ['page'] as const
/** Page data is shared by the canvas inspector and Story, keyed by project and page. */
export const pageDataKey = (name: string | null, page: PageId | null) =>
  [...pageKey, name, page] as const
export const storyKey = ['story'] as const
export const storyPageKey = (name: string | null, page: PageId) =>
  [...storyKey, name, page] as const
export const projectsKey = ['projects'] as const
export const fontsKey = ['fonts', 'installed'] as const
export const onlineFontsKey = ['fonts', 'online'] as const
export const preferencesKey = ['preferences'] as const
export const modelsKey = ['models'] as const

/** The open project as the editor lists it: its name, the page the canvas shows, its pages, and
 * its history, read through the scene's own queries. */
export interface ProjectView {
  name: string
  page: PageId | null
  pages: PageSummary[]
  history: History
  /** Each affected page's last revision, which its thumbnail follows. */
  revisions: Record<PageId, string>
}

declare module '@tanstack/react-query' {
  interface Register {
    /** `activity` is the activity center's row while the mutation runs. */
    mutationMeta: { activity?: ParseKeys }
  }
}

/** A failed command's toast: the desktop rejects with its error's message. */
function report(error: unknown): void {
  const description =
    error instanceof Error
      ? error.message
      : typeof error === 'string'
        ? error
        : i18n.t('errors.unknown')
  toast.add({ type: 'error', title: i18n.t('errors.actionFailed'), description })
}

/** Every command runs as a query or a mutation, whose failure shows here. Commands go to the
 * desktop, not the network, so they run whether or not the browser reports being online. */
export const queryClient = new QueryClient({
  queryCache: new QueryCache({ onError: report }),
  mutationCache: new MutationCache({ onError: report }),
  defaultOptions: {
    queries: {
      networkMode: 'always',
      staleTime: Number.POSITIVE_INFINITY,
      retry: false,
      refetchOnReconnect: false,
      refetchOnWindowFocus: false,
    },
    mutations: { networkMode: 'always' },
  },
})

/** Runs a command whose answer nothing waits on, as a mutation, so a failure shows as any
 * mutation's does. */
export function send<Args extends unknown[]>(
  command: (...args: Args) => Promise<unknown>,
  ...args: Args
): void {
  // TanStack Query never drops a mutation while an observer holds it, and nothing reads a
  // settled command, so let go of it once it settles and let the cache drop it right away.
  const observer = new MutationObserver(queryClient, {
    mutationFn: () => command(...args),
    gcTime: 0,
  })
  void observer
    .mutate()
    .catch(() => undefined)
    .finally(() => observer.reset())
}

const projectQuery = queryOptions({
  queryKey: projectKey,
  queryFn: async (): Promise<ProjectView | null> => {
    // Changes and page events carry the history and shown page. Only the summaries
    // need to be fetched again once the project is known. Merge into the latest cache
    // value, since a selection or page event may arrive while the summaries load.
    const cached = queryClient.getQueryData<ProjectView | null>(projectKey)
    if (cached) {
      const project = (await commands.query({ query: 'get_project' })) as ProjectAnswer
      const latest = queryClient.getQueryData<ProjectView | null>(projectKey)
      return latest ? { ...latest, pages: project.pages } : null
    }
    const name = await commands.getOpenProject()
    if (name === null) return null
    const [page, project, history] = await Promise.all([
      commands.getShownPage(),
      commands.query({ query: 'get_project' }),
      commands.query({ query: 'get_history' }),
    ])
    const known = queryClient.getQueryData<ProjectView | null>(projectKey)
    return {
      name,
      page,
      pages: (project as ProjectAnswer).pages,
      history: history as History,
      revisions: known?.name === name ? known.revisions : {},
    }
  },
})

const pageQuery = (name: string | null, page: PageId | null) =>
  queryOptions({
    queryKey: pageDataKey(name, page),
    queryFn: async (): Promise<Page | null> => {
      if (name === null || page === null) return null
      return (await commands.query({ query: 'get_page', page })) as Page
    },
  })

const fontsQuery = queryOptions({
  queryKey: fontsKey,
  queryFn: () => commands.listInstalledFonts(),
})

const onlineFontsQuery = queryOptions({
  queryKey: onlineFontsKey,
  queryFn: () => commands.listOnlineFonts(),
})

/** The open project; changes supply history and page events supply the shown page. */
export function useProject(enabled = true) {
  return useQuery({ ...projectQuery, enabled })
}

/** The open project's pages, in order. */
export function usePages(enabled = true) {
  return { data: useProject(enabled).data?.pages }
}

/** The page the editor shows, with its layers. */
export function usePage(enabled = true) {
  const { data: shown } = useQuery({
    ...projectQuery,
    enabled,
    select: (project) => [project?.name ?? null, project?.page ?? null] as const,
  })
  const [name, page] = shown ?? [null, null]
  return useQuery({ ...pageQuery(name, page), enabled: enabled && page !== null })
}

/** Story observes the same page query as the inspector, and reads its order separately. */
export function usePageStory(page: PageId, enabled = true) {
  const { data: name = null } = useQuery({
    ...projectQuery,
    enabled,
    select: (project) => project?.name ?? null,
  })
  const layers = useQuery({ ...pageQuery(name, page), enabled: enabled && name !== null }).data
  const story = useQuery({
    queryKey: storyPageKey(name, page),
    queryFn: async () => (await commands.query({ query: 'get_story', page })) as LayerId[],
    enabled: enabled && name !== null,
  }).data
  const data = useMemo(
    () => (layers && story ? { page: layers, story } : undefined),
    [layers, story],
  )
  return { data }
}

/** Installed fonts only; looking at this list never downloads the online catalog. */
export function useFonts(enabled = true) {
  return useQuery({ ...fontsQuery, enabled })
}

/** The online catalog is fetched only if a picker opens its Online Fonts tab. */
export function useOnlineFonts(enabled = false) {
  return useQuery({ ...onlineFontsQuery, enabled })
}

export function usePreferences() {
  return useQuery({ queryKey: preferencesKey, queryFn: () => commands.getPreferences() })
}

/** Saves the preferences one save after another. Each save shows at once; what the file holds
 * replaces it only while the cache still holds that save, so an answer never undoes a newer
 * edit, saved or still waiting to be. */
export function useSavePreferences() {
  return useMutation({
    mutationKey: preferencesKey,
    scope: { id: 'preferences' },
    mutationFn: (preferences: Preferences) => commands.savePreferences(preferences),
    onMutate: async (preferences) => {
      await queryClient.cancelQueries({ queryKey: preferencesKey })
      queryClient.setQueryData(preferencesKey, preferences)
    },
    onSettled: (saved, _, preferences) => {
      if (queryClient.getQueryData(preferencesKey) !== preferences) return
      if (saved) queryClient.setQueryData(preferencesKey, saved)
      else void queryClient.invalidateQueries({ queryKey: preferencesKey })
    },
  })
}

/** Edits the preferences as a settings page does: each edit shows at once, and saves once the
 * edits pause; `flush` saves one still waiting. */
export function useEditPreferences() {
  const { mutate } = useSavePreferences()
  const save = useDebouncedCallback(mutate, 260, { flushOnExit: true })
  const edit = (preferences: Preferences) => {
    void queryClient.cancelQueries({ queryKey: preferencesKey })
    queryClient.setQueryData(preferencesKey, preferences)
    save(preferences)
  }
  return { edit, flush: save.flush }
}

/** The models the translator can use. */
export function useModels() {
  return useQuery({ queryKey: modelsKey, queryFn: () => commands.listModels() })
}

/** The projects the start view lists, read again each time it shows. */
export function useProjects() {
  return useQuery({
    queryKey: projectsKey,
    queryFn: () => commands.listProjects(),
    refetchOnMount: 'always',
  })
}

export function useFontPreview(font: FontFamily | undefined, enabled = true) {
  return useQuery({
    queryKey: ['font-preview', font?.name],
    queryFn: async () => {
      if (!font) return null
      try {
        return new Uint8Array(await commands.getFontPreview(font.name))
      } catch {
        return null
      }
    },
    enabled: enabled && font !== undefined,
    gcTime: 5 * 60 * 1000,
  })
}

/** A command the activity center lists by `label` while it runs, busy wherever it's used. */
export function useCommand<Args extends unknown[], Result>(
  key: QueryKey,
  command: (...args: Args) => Promise<Result>,
  label: ParseKeys,
) {
  const busy = useIsMutating({ mutationKey: key }) > 0
  const mutation = useMutation({
    mutationKey: key,
    mutationFn: (args: Args) => command(...args),
    meta: { activity: label },
  })
  return { run: (...args: Args) => mutation.mutate(args), busy }
}

/** Imports pages; the project's change arrives as an event. */
export function useImportPages() {
  const { run, busy } = useCommand(['import-pages'], commands.import, 'navigator.importing')
  return { importPages: run, importing: busy }
}

/** Resolves once the shown page has been read since the call: a read already under way, as a
 * change starts one, is joined rather than started again, and one a later change starts is waited
 * for too. An edit shown before it lands holds until then, so it doesn't flash back. */
export async function readPage(): Promise<void> {
  const project = queryClient.getQueryData<ProjectView | null>(projectKey)
  if (!project?.page) return
  const filters = { queryKey: pageDataKey(project.name, project.page), exact: true }
  do await queryClient.refetchQueries(filters, { cancelRefetch: false })
  while (queryClient.isFetching(filters) > 0)
}

export async function refresh(...keys: QueryKey[]): Promise<void> {
  await Promise.all(keys.map((queryKey) => queryClient.invalidateQueries({ queryKey })))
}
