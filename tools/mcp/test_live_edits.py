import asyncio
import json
import os
import sys
import unittest
from pathlib import Path

from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client

SERVER = Path(__file__).with_name('server.py')
TOKEN = 'a' * 32


class LiveElementEditsTest(unittest.IsolatedAsyncioTestCase):
    async def test_element_commands_forward_document_and_revision_guards(self):
        requests = []

        async def handle(reader, writer):
            requests.append(json.loads(await reader.readline()))
            writer.write(b'{"ok":true,"result":{"document_id":"doc","revision":7}}\n')
            await writer.drain()
            writer.close()
            await writer.wait_closed()

        bridge = await asyncio.start_server(handle, '127.0.0.1', 0)
        port = str(bridge.sockets[0].getsockname()[1])
        params = StdioServerParameters(
            command=sys.executable, args=[str(SERVER)],
            env={**os.environ, 'CREABIM_MCP_PORT': port, 'CREABIM_MCP_TOKEN': TOKEN},
        )
        document = {'document_id': 'doc'}
        edit = {**document, 'expected_revision': 7}
        cases = (
            ('inspect', 'inspect_document', {}, {'offset': 1, 'limit': 10, 'kind': 'Wall'}),
            ('get_element', 'get_element', document, {'element_id': 'wall'}),
            ('move_element', 'move_element', edit, {'element_id': 'wall', 'delta': [2, 3, 0]}),
            ('delete_element', 'delete_element', edit, {'element_id': 'wall'}),
            ('undo', 'undo', edit, {}),
        )
        async with bridge, stdio_client(params) as (reader, writer):
            async with ClientSession(reader, writer) as client:
                await client.initialize()
                tools = {tool.name: tool for tool in (await client.list_tools()).tools}
                self.assertTrue(tools['live_get_element'].annotations.readOnlyHint)
                for action in ('move_element', 'delete_element', 'undo'):
                    self.assertTrue(tools[f'live_{action}'].annotations.destructiveHint)
                for tool, command, guards, args in cases:
                    with self.subTest(tool=tool):
                        result = await client.call_tool(f'live_{tool}', {**guards, **args})
                        self.assertFalse(result.isError, result)
                        self.assertEqual(requests[-1], {
                            'token': TOKEN, 'command': command, 'args': args, **guards,
                        })
