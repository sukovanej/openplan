import { QueryClientProvider } from "@tanstack/react-query"
import { lazy, StrictMode, Suspense } from "react"
import { createRoot } from "react-dom/client"
import { createBrowserRouter, RouterProvider } from "react-router-dom"

import { ACTIVITY_ROUTE, BOARD_ROUTE, FLOW_ROUTE, TAGS_ROUTE, TASK_ROUTE } from "@openplan/task-ui"
import { Skeleton } from "@openplan/ui"

import { App } from "./App"
import { queryClient } from "./lib/query-client"
import { startRealtime } from "./lib/realtime"
import { ActivityRoute } from "./routes/activity"
import { DetailRoute } from "./routes/detail"
import { ListRoute } from "./routes/list"
import { TagsRoute } from "./routes/tags"

import "@fontsource-variable/source-serif-4/index.css"
import "./index.css"

// The graph library is for the flow alone, so it loads when a reader opens the flow.
const FlowRoute = lazy(() => import("./routes/flow").then((module) => ({ default: module.FlowRoute })))

const router = createBrowserRouter([
  {
    element: <App />,
    children: [
      { path: "/", element: <ListRoute /> },
      {
        path: FLOW_ROUTE,
        element: (
          <Suspense fallback={<Skeleton className="h-full" />}>
            <FlowRoute />
          </Suspense>
        ),
      },
      { path: BOARD_ROUTE, element: <ListRoute /> },
      { path: TAGS_ROUTE, element: <TagsRoute /> },
      { path: ACTIVITY_ROUTE, element: <ActivityRoute /> },
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
