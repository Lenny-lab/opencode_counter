import { Suspense, lazy, useEffect, useState } from "react";

import { currentWindowLabel } from "@/lib/ipc";
import Popover from "@/components/Popover";
import { Spinner } from "@/components/ui/primitives";

/**
 * One bundle serves both windows, so the entry point has to work out which
 * surface it is running as. The label decides.
 *
 * The dashboard is code-split because it owns the charting library, which is
 * most of the bundle. The popover opens on a single click and must not pay for
 * charts it never draws, so it keeps the small path and the heavy one arrives
 * as a second chunk only when the dashboard window actually loads.
 */
const Dashboard = lazy(() => import("@/components/Dashboard"));

export default function App() {
  const [label, setLabel] = useState<string | null>(null);

  useEffect(() => {
    void currentWindowLabel().then(setLabel);
  }, []);

  if (label === null) {
    return (
      <div className="flex h-full items-center justify-center text-muted">
        <Spinner />
      </div>
    );
  }

  if (label === "popover") return <Popover />;

  return (
    <Suspense
      fallback={
        <div className="flex h-full items-center justify-center text-muted">
          <Spinner />
        </div>
      }
    >
      <Dashboard />
    </Suspense>
  );
}
