import { Providers } from '@/app/providers';
import { Router } from '@/app/router';

export function App(): React.ReactElement {
  return (
    <Providers>
      <Router />
    </Providers>
  );
}
