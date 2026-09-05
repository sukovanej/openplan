import { QueryClientProvider } from "@tanstack/react-query"
import { StrictMode } from "react"
import { createRoot } from "react-dom/client"
import { createBrowserRouter, RouterProvider } from "react-router-dom"

import { BOARD_ROUTE, FLOW_ROUTE, TAGS_ROUTE, TASK_ROUTE } from "@openplan/task-ui"

import { App } from "./App"
import { queryClient } from "./lib/query-client"
import { startRealtime } from "./lib/realtime"
import { DetailRoute } from "./routes/detail"
import { FlowRoute } from "./routes/flow"
import { ListRoute } from "./routes/list"
import { TagsRoute } from "./routes/tags"

import "@fontsource-variable/geist/index.css"
import "@fontsource-variable/geist-mono/index.css"
import "@fontsource-variable/source-serif-4/index.css"
import "./index.css"

const router = createBrowserRouter([
  {
    element: <App />,
    children: [
      { path: "/", element: <ListRoute /> },
      { path: FLOW_ROUTE, element: <FlowRoute /> },
      { path: BOARD_ROUTE, element: <ListRoute /> },
      { path: TAGS_ROUTE, element: <TagsRoute /> },
      { path: TASK_ROUTE, element: <DetailRoute /> },
    ],
  },
])

const root = document.getElementById("root")
if (root === null) throw new Error("missing #root element")

startRealtime()

createRoot(root).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  </StrictMode>,
)
