import { QueryClientProvider } from '@tanstack/react-query'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import { TypeBar } from '@/components/editor/TypeBar'
import { fontsKey, pageKey, preferencesKey, queryClient } from '@/lib/queries'
import { useKoharuStore } from '@/lib/store'
import { commands, type Change, type Layer, type Op } from '@koharu/ipc/protocol'

const bounds = { x: 0, y: 0, width: 80, height: 40 }
const content = [{ insert: 'Hello' }]
const base = { visible: true, locked: false, opacity: 1, blend: 'normal', attributes: {} } as const

/** Selects a balloon's translation in a path and one outside balloons in a box, both fitted by
 * `fit`, then a growing box, point text, and a block not translated yet, and picks `choice` in
 * the type bar's font size menu. Returns the edits it sent, filtered by label. */
async function chooseSize(
  fit: 'none' | 'best' | 'shrink' | 'proportional',
  choice: string,
  signFit = fit,
): Promise<(label?: string) => Op[][]> {
  const layers: Layer[] = [
    {
      ...base,
      id: 'balloon',
      name: 'balloon',
      kind: 'block',
      source: { content },
      translation: { content, frame: { path: { path: 'M0 0H80V40H0Z', fit } } },
    },
    {
      ...base,
      id: 'sign',
      name: 'sign',
      kind: 'block',
      source: { content },
      translation: { content, frame: { box: { bounds, fit: signFit } } },
    },
    {
      ...base,
      id: 'note',
      name: 'note',
      kind: 'text',
      content,
      frame: { box: { bounds, fit: 'grow' } },
    },
    { ...base, id: 'caption', name: 'caption', kind: 'text', content, frame: 'point' },
    { ...base, id: 'untranslated', name: 'untranslated', kind: 'block', source: { content } },
  ]
  queryClient.setQueryData(pageKey, { id: 'p', size: bounds, background: null, layers })
  queryClient.setQueryData(preferencesKey, { renderer: {} })
  queryClient.setQueryData(fontsKey, [])
  useKoharuStore.setState({ selectedLayers: layers.map(({ id }) => id), selectionBounds: bounds })
  const edit = vi.spyOn(commands, 'edit').mockResolvedValue({
    revision: '2',
    created: [],
    layers: [],
    pages: { added: [], removed: [], changed: [], reordered: false },
    history: { current: 1, steps: [] },
  } satisfies Change)
  render(
    <QueryClientProvider client={queryClient}>
      <TypeBar />
    </QueryClientProvider>,
  )
  fireEvent.click(screen.getByLabelText('Choose font size'))
  fireEvent.click(await screen.findByRole('menuitemradio', { name: choice }))
  return (kind = 'update_layer') =>
    edit.mock.calls.filter(([{ label }]) => label === kind).map(([{ ops }]) => ops)
}

const translated = (layer: string, frame: 'box' | 'path', fit: string): Op => ({
  op: 'update_layer',
  layer,
  patch: { translation: { frame: { [frame]: { fit } } } },
})

describe('type bar font size', () => {
  it.each(['none', 'best', 'shrink', 'proportional'] as const)(
    'sends a size-only edit for %s boxes and paths (host resolves Best Fit)',
    async (fit) => {
      const edits = await chooseSize(fit, '36')
      await waitFor(() => expect(edits('set_style')).toHaveLength(1))
      expect(edits('set_style')[0]).toEqual(
        expect.arrayContaining([
          {
            op: 'update_layer',
            layer: 'balloon',
            patch: { translation: { style: { chars: { size: 36 } } } },
          },
          {
            op: 'update_layer',
            layer: 'sign',
            patch: { translation: { style: { chars: { size: 36 } } } },
          },
          { op: 'update_layer', layer: 'note', patch: { style: { chars: { size: 36 } } } },
        ]),
      )
      expect(edits()).toEqual([])
    },
  )

  it('sends a single size edit for a mixed proportional and Best Fit selection', async () => {
    const edits = await chooseSize('proportional', '36', 'best')
    await waitFor(() => expect(edits('set_style')).toHaveLength(1))
    expect(edits()).toEqual([])
  })

  it('fits each frame, a box or a path, for Auto, leaving texts without one alone', async () => {
    const frameEdits = await chooseSize('none', 'Auto')
    await waitFor(() =>
      expect(frameEdits()).toEqual([
        [
          translated('balloon', 'path', 'best'),
          translated('sign', 'box', 'best'),
          { op: 'update_layer', layer: 'note', patch: { frame: { box: { fit: 'best' } } } },
        ],
      ]),
    )
  })
})

/** The native frame has already fitted a nominal 12 px text at 2.5×. */
function fittedTypeBar(typing = false) {
  const layer: Layer = {
    ...base,
    id: 'fitted',
    name: 'fitted',
    kind: 'text',
    content,
    frame: { box: { bounds, fit: 'best' } },
    style: { chars: { size: 12, stroke_width: 3, stroke: [1, 1, 1, 1] } },
  }
  queryClient.setQueryData(pageKey, { id: 'p', size: bounds, background: null, layers: [layer] })
  queryClient.setQueryData(preferencesKey, { renderer: {} })
  queryClient.setQueryData(fontsKey, [])
  useKoharuStore.setState({
    selectedLayers: [layer.id],
    selectionBounds: bounds,
    selectionScales: [[layer.id, 2.5]],
    // The live text may have a different fitted scale from the committed snapshot.
    typing: typing
      ? { layer: layer.id, chars: layer.style!.chars!, mixed: [], scale: 2 }
      : null,
  })
  render(
    <QueryClientProvider client={queryClient}>
      <TypeBar />
    </QueryClientProvider>,
  )
}

describe('rendered type bar values', () => {
  it('shows the fitted size and outline width, and writes the nominal width on editing', async () => {
    const edit = vi.spyOn(commands, 'edit').mockResolvedValue({
      revision: '2', created: [], layers: [],
      pages: { added: [], removed: [], changed: [], reordered: false },
      history: { current: 1, steps: [] },
    } satisfies Change)
    fittedTypeBar()
    expect(screen.getByTestId('type-size')).toHaveAttribute('placeholder', '30 (Auto)')
    expect(screen.getByLabelText('Border width')).toHaveDisplayValue('7.5')
    fireEvent.click(screen.getByLabelText('Choose border width'))
    fireEvent.click(await screen.findByRole('menuitemradio', { name: '3' }))
    await waitFor(() => expect(edit).toHaveBeenCalledWith({
      label: 'set_style',
      ops: [{ op: 'update_layer', layer: 'fitted', patch: {
        style: { chars: { stroke_width: 1.2, stroke: [1, 1, 1, 1] } },
      } }],
    }))
  })

  it('sets paragraph line spacing without changing the fitted font size', async () => {
    const edit = vi.spyOn(commands, 'edit').mockResolvedValue({
      revision: '2', created: [], layers: [],
      pages: { added: [], removed: [], changed: [], reordered: false },
      history: { current: 1, steps: [] },
    } satisfies Change)
    fittedTypeBar()
    expect(screen.getByLabelText('Line spacing (%)')).toHaveDisplayValue('120')
    fireEvent.click(screen.getByLabelText('Choose line spacing'))
    fireEvent.click(await screen.findByRole('menuitemradio', { name: '150' }))
    await waitFor(() => expect(edit).toHaveBeenCalledWith({
      label: 'set_style',
      ops: [{ op: 'update_layer', layer: 'fitted', patch: { style: { line_height: 1.5 } } }],
    }))
  })

  it('adds the second outline independently in fitted pixels', async () => {
    const edit = vi.spyOn(commands, 'edit').mockResolvedValue({
      revision: '2', created: [], layers: [],
      pages: { added: [], removed: [], changed: [], reordered: false },
      history: { current: 1, steps: [] },
    } satisfies Change)
    fittedTypeBar()
    fireEvent.click(screen.getByLabelText('Choose second outline width'))
    fireEvent.click(await screen.findByRole('menuitemradio', { name: '5' }))
    await waitFor(() => expect(edit).toHaveBeenCalledWith({
      label: 'set_style',
      ops: [{ op: 'update_layer', layer: 'fitted', patch: {
        style: { chars: { outer_stroke_width: 2, outer_stroke: [0, 0, 0, 1] } },
      } }],
    }))
  })

  it('uses the live canvas layout rather than the committed snapshot while typing', async () => {
    const style = vi.spyOn(commands, 'styleTyping').mockResolvedValue(null)
    fittedTypeBar(true)
    expect(screen.getByTestId('type-size')).toHaveAttribute('placeholder', '24 (Auto)')
    expect(screen.getByLabelText('Border width')).toHaveDisplayValue('6')
    fireEvent.click(screen.getByLabelText('Choose border width'))
    fireEvent.click(await screen.findByRole('menuitemradio', { name: '3' }))
    await waitFor(() => expect(style).toHaveBeenCalledWith({
      stroke_width: 1.5, stroke: [1, 1, 1, 1],
    }))
    fireEvent.click(screen.getByLabelText('Choose second outline width'))
    fireEvent.click(await screen.findByRole('menuitemradio', { name: '4' }))
    await waitFor(() => expect(style).toHaveBeenCalledWith({
      outer_stroke_width: 2, outer_stroke: [0, 0, 0, 1],
    }))
  })
})
