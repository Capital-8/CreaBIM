import asyncio
import json
import os
from pathlib import Path
import sys
import unittest

from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client

SERVER = Path(__file__).with_name('server.py')
TOKEN = 'a' * 32


class LiveStructureTest(unittest.IsolatedAsyncioTestCase):
    async def test_structural_tools_send_native_commands(self):
        requests = []

        async def handle(reader, writer):
            try:
                requests.append(json.loads(await reader.readline()))
                writer.write(b'{"ok":true,"result":{"revision":8}}\n')
                await writer.drain()
            finally:
                writer.close()
                await writer.wait_closed()

        bridge = await asyncio.start_server(handle, '127.0.0.1', 0)
        edit = {'document_id': 'doc', 'expected_revision': 7}
        commands = {
            'create_level': {'name': 'Mezzanine', 'elevation': 4.0, 'height': 3.2},
            'create_column': {'level_id': 'level', 'name': 'Column', 'position': [8, 0],
                              'width': 0.3, 'depth': 0.3, 'height': 2.6},
            'create_beam': {'level_id': 'level', 'name': 'Beam', 'start': [8, 0], 'end': [12, 0],
                            'width': 0.3, 'depth': 0.4, 'top_offset': 0.15},
        }
        try:
            params = StdioServerParameters(
                command=sys.executable,
                args=[str(SERVER)],
                env={**os.environ, 'CREABIM_MCP_PORT': str(bridge.sockets[0].getsockname()[1]),
                     'CREABIM_MCP_TOKEN': TOKEN},
            )
            async with stdio_client(params) as (reader, writer):
                async with ClientSession(reader, writer) as client:
                    await client.initialize()
                    tools = {tool.name for tool in (await client.list_tools()).tools}
                    self.assertTrue({f'live_{command}' for command in commands} <= tools)
                    for command, args in commands.items():
                        with self.subTest(command=command):
                            result = await client.call_tool(f'live_{command}', {**edit, **args})
                            self.assertFalse(result.isError, result)
                            self.assertEqual(requests[-1], {
                                'token': TOKEN, 'command': command, 'args': args, **edit,
                            })
        finally:
            bridge.close()
            await bridge.wait_closed()
