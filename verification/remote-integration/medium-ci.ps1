$ErrorActionPreference='Stop'
if($env:GITHUB_ACTIONS -ne 'true'){throw 'Ephemeral Windows CI only'}
# runas /trustlevel removes administrative groups, but may retain a high
# mandatory label. Lower only this helper's own token before creating Node.
# No other process, registry, user right or system policy is changed.
Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class OwnedMediumToken {
  [StructLayout(LayoutKind.Sequential)] struct Label { public IntPtr Sid; public uint Attributes; }
  [DllImport("kernel32.dll")] static extern IntPtr GetCurrentProcess();
  [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr value);
  [DllImport("kernel32.dll")] static extern IntPtr LocalFree(IntPtr value);
  [DllImport("advapi32.dll",SetLastError=true)] static extern bool OpenProcessToken(IntPtr process,uint access,out IntPtr token);
  [DllImport("advapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool ConvertStringSidToSid(string text,out IntPtr sid);
  [DllImport("advapi32.dll")] static extern uint GetLengthSid(IntPtr sid);
  [DllImport("advapi32.dll",SetLastError=true)] static extern bool SetTokenInformation(IntPtr token,int info,IntPtr data,int size);
  public static void LowerSelf() {
    IntPtr token=IntPtr.Zero,sid=IntPtr.Zero,buffer=IntPtr.Zero;
    try {
      if(!OpenProcessToken(GetCurrentProcess(),0x80|0x08,out token))throw new Win32Exception(Marshal.GetLastWin32Error());
      if(!ConvertStringSidToSid("S-1-16-8192",out sid))throw new Win32Exception(Marshal.GetLastWin32Error());
      var value=new Label {Sid=sid,Attributes=0x20};
      int size=Marshal.SizeOf(typeof(Label)); buffer=Marshal.AllocHGlobal(size);
      Marshal.StructureToPtr(value,buffer,false);
      if(!SetTokenInformation(token,25,buffer,size+(int)GetLengthSid(sid)))throw new Win32Exception(Marshal.GetLastWin32Error());
    } finally {if(buffer!=IntPtr.Zero)Marshal.FreeHGlobal(buffer);if(sid!=IntPtr.Zero)LocalFree(sid);if(token!=IntPtr.Zero)CloseHandle(token);}
  }
}
'@
[OwnedMediumToken]::LowerSelf()
$nodeExe=(Get-Command node.exe).Source
& $nodeExe (Join-Path $PSScriptRoot 'native-ui.mjs')
exit $LASTEXITCODE
