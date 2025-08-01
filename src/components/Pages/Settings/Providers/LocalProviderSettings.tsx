import { PlusOutlined, UploadOutlined } from '@ant-design/icons'
import { App, Button, Card, Divider, Dropdown, Flex, Form } from 'antd'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useParams } from 'react-router-dom'
import { isDesktopApp } from '../../../../api/core'
import { Permission, usePermissions } from '../../../../permissions'
import {
  clearProvidersError,
  deleteExistingModel,
  disableModelFromUse,
  enableModelForUse,
  loadModels,
  openAddLocalModelDownloadDrawer,
  openAddLocalModelUploadDrawer,
  openEditLocalModelDrawer,
  startModelExecution,
  stopModelExecution,
  Stores,
  updateModelProvider,
} from '../../../../store'
import { DownloadInstance, Provider } from '../../../../types'
import { DownloadItem } from '../../../shared/DownloadItem'
import { ModelsSection } from './shared/ModelsSection'
import { ProviderHeader } from './shared/ProviderHeader'

export function LocalProviderSettings() {
  const { t } = useTranslation()
  const { message } = App.useApp()
  const { hasPermission } = usePermissions()
  const { provider_id } = useParams<{ provider_id?: string }>()

  const [nameForm] = Form.useForm()
  const [isMobile, setIsMobile] = useState(false)

  // Store data
  const { providers, modelsByProvider, loadingModels, modelOperations, error } =
    Stores.AdminProviders
  const { downloads } = Stores.ModelDownload

  // Check permissions for web app
  const canEditProviders =
    isDesktopApp || hasPermission(Permission.config.providers.edit)

  // Find current provider
  const currentProvider = providers.find(p => p.id === provider_id)
  const currentModels = provider_id ? modelsByProvider[provider_id] || [] : []
  const modelsLoading = provider_id
    ? loadingModels[provider_id] || false
    : false

  // Get active downloads for this provider
  const providerDownloads = Object.values(downloads).filter(
    (download: DownloadInstance) => download.provider_id === provider_id,
  )

  // Helper functions for provider validation
  const canEnableProvider = (provider: Provider): boolean => {
    if (provider.enabled) return true // Already enabled
    if (provider.type === 'local') return true
    if (!provider.api_key || provider.api_key.trim() === '') return false
    if (!provider.base_url || provider.base_url.trim() === '') return false
    try {
      new globalThis.URL(provider.base_url)
      return true
    } catch {
      return false
    }
  }

  const getEnableDisabledReason = (provider: Provider): string | null => {
    if (provider.enabled) return null
    if (provider.type === 'local') return null
    if (!provider.api_key || provider.api_key.trim() === '')
      return 'API key is required'
    if (!provider.base_url || provider.base_url.trim() === '')
      return 'Base URL is required'
    try {
      new globalThis.URL(provider.base_url)
      return null
    } catch {
      return 'Invalid base URL format'
    }
  }

  // Event handlers
  const handleNameChange = async (changedValues: any) => {
    if (!currentProvider || !canEditProviders) return

    try {
      await updateModelProvider(currentProvider.id, {
        name: changedValues.name,
      })
    } catch (error) {
      console.error('Failed to update provider:', error)
      // Error is handled by the store
    }
  }

  const handleProviderToggle = async (providerId: string, enabled: boolean) => {
    if (!canEditProviders) {
      message.error(t('providers.noPermissionModify'))
      return
    }

    try {
      await updateModelProvider(providerId, {
        enabled: enabled,
      })
      const provider = providers.find(p => p.id === providerId)
      message.success(
        `${provider?.name || 'Provider'} ${enabled ? 'enabled' : 'disabled'}`,
      )
    } catch (error: any) {
      console.error('Failed to update provider:', error)
      // Handle error similar to original implementation
      if (error.response?.status === 400) {
        const provider = providers.find(p => p.id === providerId)
        if (provider) {
          const providerModels = modelsByProvider[provider.id] || []
          if (providerModels.length === 0) {
            message.error(
              `Cannot enable "${provider.name}" - No models available`,
            )
          } else {
            message.error(
              `Cannot enable "${provider.name}" - Invalid configuration`,
            )
          }
        } else {
          message.error(error?.message || 'Failed to update provider')
        }
      } else {
        message.error(error?.message || 'Failed to update provider')
      }
    }
  }

  const handleToggleModel = async (modelId: string, enabled: boolean) => {
    if (!currentProvider) return

    try {
      if (enabled) {
        await enableModelForUse(modelId)
      } else {
        await disableModelFromUse(modelId)
      }

      // Check if this was the last enabled model being disabled
      if (!enabled) {
        const providerModels = currentModels
        const remainingEnabledModels = providerModels.filter(
          m => m.id !== modelId && m.enabled !== false,
        )

        // If no models remain enabled and provider is currently enabled, disable the provider
        if (remainingEnabledModels.length === 0 && currentProvider.enabled) {
          try {
            await updateModelProvider(currentProvider.id, { enabled: false })
            const modelName =
              providerModels.find(m => m.id === modelId)?.name || 'Model'
            message.success(
              `${modelName} disabled. ${currentProvider.name} provider disabled as no models remain active.`,
            )
          } catch (providerError) {
            console.error('Failed to disable provider:', providerError)
            const modelName =
              providerModels.find(m => m.id === modelId)?.name || 'Model'
            message.warning(
              `${modelName} disabled, but failed to disable provider automatically`,
            )
          }
        } else {
          const modelName =
            currentModels.find(m => m.id === modelId)?.name || 'Model'
          message.success(`${modelName} ${enabled ? 'enabled' : 'disabled'}`)
        }
      } else {
        const modelName =
          currentModels.find(m => m.id === modelId)?.name || 'Model'
        message.success(`${modelName} ${enabled ? 'enabled' : 'disabled'}`)
      }
    } catch (error) {
      console.error('Failed to toggle model:', error)
      // Error is handled by the store
    }
  }

  const handleDeleteModel = async (modelId: string) => {
    if (!currentProvider) return

    try {
      await deleteExistingModel(modelId)
      message.success(t('providers.modelDeleted'))
    } catch (error) {
      console.error('Failed to delete model:', error)
      // Error is handled by the store
    }
  }

  const handleStartStopModel = async (modelId: string, is_active: boolean) => {
    if (!currentProvider || currentProvider.type !== 'local') return

    try {
      if (is_active) {
        await startModelExecution(modelId)
      } else {
        await stopModelExecution(modelId)
      }

      const modelName =
        currentModels.find(m => m.id === modelId)?.name || 'Model'
      message.success(`${modelName} ${is_active ? 'started' : 'stopped'}`)
    } catch (error) {
      console.error('Failed to start/stop model:', error)
      // Error is handled by the store
    }
  }

  // Effects
  useEffect(() => {
    const checkMobile = () => {
      setIsMobile(window.innerWidth < 768)
    }

    checkMobile()
    window.addEventListener('resize', checkMobile)

    return () => window.removeEventListener('resize', checkMobile)
  }, [])

  // Load models when provider is selected
  useEffect(() => {
    if (
      provider_id &&
      !modelsByProvider[provider_id] &&
      !loadingModels[provider_id]
    ) {
      loadModels(provider_id)
    }
  }, [
    provider_id,
    provider_id ? modelsByProvider[provider_id] : undefined,
    provider_id ? loadingModels[provider_id] : undefined,
  ])

  // Show errors
  useEffect(() => {
    if (error) {
      message.error(error)
      clearProvidersError()
    }
  }, [error]) // Removed message from dependencies to prevent infinite rerenders

  // Update form when provider changes
  useEffect(() => {
    if (currentProvider) {
      nameForm.setFieldsValue({
        name: currentProvider.name,
      })
    }
  }, [currentProvider]) // Removed nameForm from dependencies to prevent infinite rerenders

  // Return early if no provider or not local
  if (!currentProvider || currentProvider.type !== 'local') {
    return null
  }

  const addModelButton = (
    <Dropdown
      menu={{
        items: [
          {
            key: 'upload',
            label: 'Upload from Files',
            icon: <UploadOutlined />,
            onClick: () => openAddLocalModelUploadDrawer(currentProvider.id),
          },
          {
            key: 'download',
            label: 'Download from Repository',
            icon: <PlusOutlined />,
            onClick: () => openAddLocalModelDownloadDrawer(currentProvider.id),
          },
        ],
      }}
      trigger={['click']}
    >
      <Button type="text" icon={<PlusOutlined />} />
    </Dropdown>
  )

  return (
    <Flex className={'flex-col gap-3'}>
      <ProviderHeader
        currentProvider={currentProvider}
        isMobile={isMobile}
        canEditProviders={canEditProviders}
        nameForm={nameForm}
        onNameChange={handleNameChange}
        onProviderToggle={handleProviderToggle}
        canEnableProvider={canEnableProvider}
        getEnableDisabledReason={getEnableDisabledReason}
      />

      {/* Downloads Section - For Local providers only */}
      {providerDownloads.length > 0 && (
        <Card title={t('providers.downloadingModels')}>
          <Flex vertical>
            {providerDownloads.map((download: DownloadInstance, i: number) => (
              <>
                <DownloadItem key={download.id} download={download} />
                {i < providerDownloads.length - 1 && (
                  <Divider className={'m-0'} />
                )}
              </>
            ))}
          </Flex>
        </Card>
      )}

      {/* Models Section */}
      <ModelsSection
        currentProvider={currentProvider}
        currentModels={currentModels}
        modelsLoading={modelsLoading}
        canEditProviders={canEditProviders}
        isMobile={isMobile}
        modelOperations={modelOperations}
        onAddModel={() => {
          // Not used since we have customAddButton
        }}
        onToggleModel={handleToggleModel}
        onEditModel={modelId => openEditLocalModelDrawer(modelId)}
        onDeleteModel={handleDeleteModel}
        onStartStopModel={handleStartStopModel}
        customAddButton={addModelButton}
      />
    </Flex>
  )
}
