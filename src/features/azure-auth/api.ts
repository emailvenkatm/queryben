import { useMutation, useQuery, useQueryClient, type UseQueryResult } from '@tanstack/react-query';
import {
  commands,
  type AzureAccount,
  type AccountRegistryEntry,
  type AzureSubscription,
  type AzureSqlServer,
  type AzureSqlDatabase,
} from '@/shared/api/tauri-bindings';

export { type AzureAccount, type AccountRegistryEntry, type AzureSubscription, type AzureSqlServer, type AzureSqlDatabase };

export const AZURE_ACCOUNT_KEY = ['azure', 'account'] as const;
export const AZURE_ACCOUNTS_KEY = ['azure', 'accounts'] as const;

export function useAzureAccount(): UseQueryResult<AzureAccount | null> {
  return useQuery({
    queryKey: AZURE_ACCOUNT_KEY,
    queryFn: () => commands.azureCurrentAccount(),
    staleTime: Infinity,
    gcTime: Infinity,
  });
}

export function useAzureAccounts(): UseQueryResult<AccountRegistryEntry[]> {
  return useQuery({
    queryKey: AZURE_ACCOUNTS_KEY,
    queryFn: () => commands.azureListAccounts(),
    staleTime: Infinity,
    gcTime: Infinity,
  });
}

interface AzureAuthApi {
  account: AzureAccount | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  signIn: () => Promise<AzureAccount>;
  signOut: () => Promise<void>;
  signOutAccount: (accountId: string) => Promise<void>;
  signingIn: boolean;
  signingOut: boolean;
  signInError: unknown;
}

export function useAzureAuth(): AzureAuthApi {
  const qc = useQueryClient();
  const accountQuery = useAzureAccount();
  const account = accountQuery.data ?? null;

  const signInMutation = useMutation({
    mutationFn: () => commands.azureSignIn(),
    onSuccess: (newAccount) => {
      qc.setQueryData(AZURE_ACCOUNT_KEY, newAccount);
      qc.invalidateQueries({ queryKey: ['azure'] });
    },
  });

  const signOutMutation = useMutation({
    mutationFn: () => commands.azureSignOut(),
    onSuccess: () => {
      qc.setQueryData(AZURE_ACCOUNT_KEY, null);
      qc.setQueryData(AZURE_ACCOUNTS_KEY, []);
      qc.invalidateQueries({ queryKey: ['azure'] });
    },
  });

  const signOutAccountMutation = useMutation({
    mutationFn: (accountId: string) => commands.azureSignOutAccount(accountId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['azure'] });
      qc.invalidateQueries({ queryKey: ['azure', 'has-cached-token'] });
    },
  });

  return {
    account,
    isAuthenticated: !!account,
    isLoading: accountQuery.isLoading,
    signIn: () => signInMutation.mutateAsync(),
    signOut: () => signOutMutation.mutateAsync(),
    signOutAccount: (accountId: string) => signOutAccountMutation.mutateAsync(accountId),
    signingIn: signInMutation.isPending,
    signingOut: signOutMutation.isPending,
    signInError: signInMutation.error,
  };
}

export function useAzureSubscriptions(accountId?: string): UseQueryResult<AzureSubscription[]> {
  const accountsQuery = useAzureAccounts();
  const { isAuthenticated } = useAzureAuth();
  const enabled = (accountsQuery.data ?? []).length > 0 || isAuthenticated;

  return useQuery({
    queryKey: ['azure', 'subscriptions', accountId ?? '__current__'],
    queryFn: () => commands.listAzureSubscriptions(accountId),
    enabled,
    staleTime: 5 * 60 * 1000,
  });
}

export function useAzureSqlServers(
  subscriptionId: string | null,
  accountId?: string,
): UseQueryResult<AzureSqlServer[]> {
  return useQuery({
    queryKey: ['azure', 'sql-servers', accountId ?? '__current__', subscriptionId],
    queryFn: () => commands.listAzureSqlServers(subscriptionId as string, accountId),
    enabled: !!subscriptionId,
    staleTime: 2 * 60 * 1000,
  });
}

export function useAzureSqlDatabases(
  serverId: string | null,
  accountId?: string,
): UseQueryResult<AzureSqlDatabase[]> {
  return useQuery({
    queryKey: ['azure', 'sql-databases', accountId ?? '__current__', serverId],
    queryFn: () => commands.listAzureSqlDatabases(serverId as string, accountId),
    enabled: !!serverId,
    staleTime: 2 * 60 * 1000,
  });
}
