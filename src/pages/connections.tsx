import { useState } from 'react';
import { ListScreen, AddSheet } from '@/features/connections';

export function ConnectionsPage(): React.ReactElement {
  const [adding, setAdding] = useState(false);
  return (
    <>
      <ListScreen onAddConnection={() => setAdding(true)} />
      <AddSheet open={adding} onOpenChange={setAdding} />
    </>
  );
}
