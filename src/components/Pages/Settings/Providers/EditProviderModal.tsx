import { EyeInvisibleOutlined, EyeTwoTone } from '@ant-design/icons'
import { Card, Flex, Form, Input, Modal, Switch } from 'antd'
import { useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import {
  closeEditProviderModal,
  setEditProviderModalLoading,
  Stores,
  updateModelProvider,
} from '../../../../store'
import { UpdateProviderRequest } from '../../../../types/api/provider'

export function EditProviderModal() {
  const { t } = useTranslation()
  const [form] = Form.useForm()

  const { open, loading, providerId } = Stores.UI.EditProviderModal
  const { providers } = Stores.Providers

  // Find the current provider from the store
  const provider = providerId ? providers.find(p => p.id === providerId) : null

  useEffect(() => {
    if (provider && open) {
      form.setFieldsValue({
        name: provider.name,
        enabled: provider.enabled,
        api_key: provider.api_key,
        base_url: provider.base_url,
      })
    }
  }, [provider, open, form])

  const handleSubmit = async () => {
    if (!provider) return

    try {
      setEditProviderModalLoading(true)
      const values = await form.validateFields()
      await updateModelProvider(provider.id, {
        id: provider.id,
        ...values,
      } as UpdateProviderRequest)
      closeEditProviderModal()
    } catch (error) {
      console.error('Failed to update provider:', error)
    } finally {
      setEditProviderModalLoading(false)
    }
  }

  if (!provider) return null

  return (
    <Modal
      title={`${t('providers.editProvider')} ${provider.name}`}
      open={open}
      onCancel={closeEditProviderModal}
      onOk={handleSubmit}
      confirmLoading={loading}
      width={600}
      destroyOnHidden={true}
      maskClosable={false}
    >
      <Form form={form} layout="vertical">
        <Form.Item
          name="name"
          label={t('providers.providerName')}
          rules={[
            {
              required: true,
              message: t('providers.providerNameRequired'),
            },
          ]}
        >
          <Input placeholder={t('providers.providerNamePlaceholder')} />
        </Form.Item>

        <Form.Item
          name="enabled"
          label={t('providers.enabled')}
          valuePropName="checked"
        >
          <Switch />
        </Form.Item>

        {/* API Configuration for non-local providers */}
        {provider.type !== 'local' && (
          <Flex vertical className="gap-2 w-full">
            <Card size="small" title={t('providers.apiConfiguration')}>
              <Form.Item
                name="api_key"
                label={t('providers.apiKey')}
                rules={[
                  {
                    required: true,
                    message: t('providers.apiKeyRequired'),
                  },
                ]}
              >
                <Input.Password
                  placeholder={t('providers.apiKeyPlaceholder')}
                  iconRender={visible =>
                    visible ? <EyeTwoTone /> : <EyeInvisibleOutlined />
                  }
                />
              </Form.Item>

              <Form.Item
                name="base_url"
                label={t('providers.baseUrl')}
                rules={[
                  {
                    required: true,
                    message: t('providers.baseUrlRequired'),
                  },
                ]}
              >
                <Input placeholder={t('providers.baseUrlPlaceholder')} />
              </Form.Item>
            </Card>
          </Flex>
        )}
      </Form>
    </Modal>
  )
}
