import { useEffect } from 'react';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { z } from 'zod';
import { useCreateConnection, useTestConnection } from '../api';

const connectionSchema = z
  .object({
    server: z.string().min(1, 'Server is required').max(255, 'Server name too long'),
    database: z.string().min(1, 'Database is required').max(128, 'Database name too long'),
    authMode: z.enum(['sqlAuth', 'aadToken', 'aadPassword', 'aadInteractive', 'aadManagedIdentity'], {
      required_error: 'Auth method is required',
    }),
    username: z.string().max(128).optional(),
    password: z.string().max(1024).optional(),
    environment: z.enum(['production', 'staging', 'development', 'local'], {
      required_error: 'Environment is required',
    }),
    nickname: z.string().max(40, 'Nickname is too long').optional(),
    color: z.enum(['cream', 'amber', 'jade', 'rose', 'violet', 'graphite']).nullable().optional(),
  })
  .superRefine((data, ctx) => {
    if (data.authMode === 'sqlAuth' || data.authMode === 'aadPassword') {
      if (!data.username?.trim()) {
        ctx.addIssue({ code: z.ZodIssueCode.custom, message: 'Username is required for this auth method', path: ['username'] });
      }
      if (!data.password?.trim()) {
        ctx.addIssue({ code: z.ZodIssueCode.custom, message: 'Password is required for this auth method', path: ['password'] });
      }
    }
  });

type ConnectionFormValues = z.infer<typeof connectionSchema>;

function deriveConnectionName(server: string, database: string): string {
  const host = server.split('.')[0] || server;
  return `${host}/${database}`;
}

export interface UseAddConnectionFormReturn {
  form: ReturnType<typeof useForm<ConnectionFormValues>>;
  authMode: ConnectionFormValues['authMode'];
  needsCredentials: boolean;
  createConn: ReturnType<typeof useCreateConnection>;
  testConn: ReturnType<typeof useTestConnection>;
  handleSubmit: (e?: React.BaseSyntheticEvent) => Promise<void>;
  handleTest: () => Promise<void>;
}

export function useAddConnectionForm(
  open: boolean,
  onSuccess: () => void,
): UseAddConnectionFormReturn {
  const form = useForm<ConnectionFormValues>({
    resolver: zodResolver(connectionSchema),
    defaultValues: {
      server: '',
      database: '',
      authMode: 'sqlAuth',
      username: '',
      password: '',
      environment: 'development',
      nickname: '',
      color: null,
    },
  });

  const authMode = form.watch('authMode');
  const needsCredentials = authMode === 'sqlAuth' || authMode === 'aadPassword';

  const createConn = useCreateConnection();
  const testConn = useTestConnection();

  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (!open) {
      form.reset();
      testConn.reset();
    }
  }, [open]);

  const handleSubmit = form.handleSubmit(async (values) => {
    await createConn.mutateAsync({
      name: deriveConnectionName(values.server, values.database),
      server: values.server,
      database: values.database,
      authMode: values.authMode,
      username: values.username,
      password: values.password,
      nickname: values.nickname?.trim() || null,
      color: values.color ?? null,
    });
    onSuccess();
  });

  const handleTest = async (): Promise<void> => {
    const valid = await form.trigger();
    if (!valid) return;
    const values = form.getValues();
    testConn.mutate({
      name: deriveConnectionName(values.server, values.database),
      server: values.server,
      database: values.database,
      authMode: values.authMode,
      username: values.username,
      password: values.password,
    });
  };

  return { form, authMode, needsCredentials, createConn, testConn, handleSubmit, handleTest };
}
