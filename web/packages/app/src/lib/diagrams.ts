import type { DrawDiagram } from "@openplan/task-ui"

import { DiagramRefused, drawDiagram } from "./api"
import { errorText } from "./format"
import { diagramKey, queryClient } from "./query-client"
import { abortable } from "./runtime"

export const drawDiagramOnce: DrawDiagram = (source) =>
  queryClient.fetchQuery({ queryKey: diagramKey(source), queryFn: abortable(drawDiagram(source)) }).then(
    (drawing) => ({ drawing }),
    (error: unknown) =>
      error instanceof DiagramRefused
        ? { error: error.message, line: error.position?.line }
        : { error: errorText(error) },
  )
