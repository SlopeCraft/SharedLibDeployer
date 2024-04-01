use std::path::{Path, PathBuf};
use std::process::{Command, exit};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = "Deploy dll for exe or dll.")]
struct Args {
    /// The target file to deploy dll for. This can be an exe or dll.
    binary_file: String,

    /// Do not search in system variable PATH
    #[arg(long, default_value_t = false)]
    skip_env_path: bool,

    /// Copy Microsoft Visual C/C++ redistributable dlls.
    #[arg(long, default_value_t = false)]
    copy_vc_redist: bool,

    /// Show verbose information during execution
    #[arg(long, default_value_t = false)]
    verbose: bool,

    /// Search for dll in those dirs
    #[arg(long)]
    shallow_search_dir: Vec<String>,
    /// Disable shallow search
    #[arg(long, default_value_t = false)]
    no_shallow_search: bool,

    /// Search for dll recursively in those dirs
    #[arg(long)]
    deep_search_dir: Vec<String>,
    /// Disable recursive search
    #[arg(long, default_value_t = false)]
    no_deep_search: bool,

    /// CMAKE_PREFIX_PATH for cmake to search for packages
    #[arg(long)]
    cmake_prefix_path: Vec<String>,
    /// Dll files that won't be deployed
    #[arg(long)]
    ignore: Vec<String>,

    /// Location of dumpbin file. Valid values: [auto] [system] [builtin] path
    #[arg(long, default_value_t = String::from("[auto]"))]
    objdump_file: String,
    /// If one or more dll failed to be found, skip it and go on
    #[arg(long, default_value_t = false)]
    allow_missing: bool,
}

fn existing_var_path(dest: &mut Vec<String>) {
    if let Ok(path) = std::env::var("PATH") {
        for path in path.split(';') {
            if !can_be_dir(&path) {
                continue;
            }
            dest.push(path.to_string());
        }
    }
}

fn get_system_objdump()->Option<String> {
    let output=Command::new("where").args(["objdump"]).output().unwrap();
    let output=String::from_utf8(output.stdout).unwrap().replace('\r',"");
    for line in output.split('\n') {
        if is_file(&line) {
            return Some(line.to_string());
        }
    }
    return None;
}

fn get_objdump_file(input:&str)->String {
    if input=="[system]" {
        if let Some(loc)=get_system_objdump() {
            return loc;
        }
        eprintln!("Failed to find objdump in your system");
        exit(2);
    }

    if input == "[builtin]" {
        let current_exe = std::env::current_exe().expect("Get current exe name");
        //println!("current_exe = {}",current_exe.to_str().unwrap());
        let install_prefix = current_exe.parent().expect("Get parent dir of current exe");
        //println!("install_prefix = {}",install_prefix.to_str().unwrap());
        let mut p = install_prefix.to_path_buf();
        if cfg!(target_os = "windows"){
            p.push("objdump.exe");
        }else {
            p.push("objdump");
        }

        if !is_file(&p) {
            eprintln!("Builtin objdump executable {} not found",p.display());
            exit(3);
        }

        //println!("objdump path = {}",p.to_str().unwrap());
        return p.to_str().unwrap().to_string();
    }

    if input=="[auto]"  {
        if let Some(loc)=get_system_objdump() {
            return loc;
        }
        return get_objdump_file("[builtin]");
    }

    if !is_file(&input) {
        eprintln!("Given objdump file {} doesn't exist",input);
        exit(4);
    }

    return input.to_string();
}

impl Args {
    fn objdump_file(&self) -> String {
        return get_objdump_file(&self.objdump_file);
    }

    fn shallow_search_dirs(&self) -> Vec<String> {
        let mut vec = self.shallow_search_dir.clone();
        self.existing_cmake_prefix_path(&mut vec);

        if cfg!(target_os = "windows") && !self.skip_env_path {
            existing_var_path(&mut vec);
        }

        return vec;
    }

    fn existing_cmake_prefix_path(&self, dest: &mut Vec<String>) {
        for path in &self.cmake_prefix_path {
            for path in path.split(';') {
                let path = format!("{path}/bin");
                if can_be_dir(&path) {
                    dest.push(path);
                }
            }
        }
    }

    fn deep_search_dirs(&self) -> Vec<String> {
        let mut vec = self.deep_search_dir.clone();
        self.existing_cmake_prefix_path(&mut vec);

        if cfg!(target_os = "windows") && !self.skip_env_path {
            existing_var_path(&mut vec);
        }

        return vec;
    }
}

fn parse_output_single_line(output: &str) -> &str {
    let fail_msg = format!("Failed to parse dll name from output \"{output}\"");
    let loc1 = output.find("dll name: ").expect(&fail_msg);
    let loc2 = output.find(".dll").expect(&fail_msg);

    let loc1 = loc1 + "dll name: ".len();
    if loc1 + 1 >= loc2 {
        eprintln!("{}", fail_msg);
        exit(8);
    }

    return &output[loc1..loc2];
}

fn get_dependencies(file: &str, objdump_file: &str) -> Vec<String> {
    let output = Command::new(objdump_file).args([file, "-x", "--section=.rdata"]).output()
        .expect(&format!("Failed to run objdump at {}", objdump_file));

    if !output.status.success() {
        eprintln!("{} {} -x failed with error code {}", objdump_file, file, output.status.to_string());
        eprintln!("The std error is: {}", String::from_utf8(output.stderr).unwrap());
        exit(1);
    }

    let output = String::from_utf8(output.stdout)
        .expect("Failed to convert output to utf8")
        .replace('\r',"")
        .to_lowercase();
    let split = output.split("\n");
    let mut dlls = Vec::with_capacity(split.clone().count());
    //let regex=Regex::new(r"dll name: (.+)\.dll").unwrap();
    for line in split {
        if !line.contains("dll name: ") {
            continue;
        }

        let mut str = parse_output_single_line(line).to_string();
        str.push_str(".dll");
        dlls.push(str);
    }
    return dlls;
}

fn is_vc_redist_dll(name: &str) -> bool {
    return name.starts_with("api-ms-win");
}

fn is_system_dll(name: &str) -> bool {
    return if cfg!(target_os = "windows") {
        let system_prefices = [
            "C:/Windows/",
            "C:/Windows/system32/",
            "C:/Windows/System32/Wbem/",
            "C:/Windows/System32/WindowsPowerShell/v1.0/",
            "C:/Windows/System32/OpenSSH/"];
        for prefix in system_prefices {
            let filename = format!("{prefix}{name}");
            if is_file(&filename) {
                return true;
            }
        }

        false
    } else {
        // Fallback solution for cross compiling
        const SYSTEM_DLL_LIST: [&str; 3392] = ["07409496-a423-4a3e-b620-2cfb01a9318d_hyperv-computenetwork.dll", "0ae3b998-9a38-4b72-a4c4-06849441518d_servicing-stack.dll", "4545ffe2-0dc4-4df4-9d02-299ef204635e_hvsocket.dll", "69fe178f-26e7-43a9-aa7d-2b616b672dde_eventlogservice.dll", "6bea57fb-8dfb-4177-9ae8-42e8b3529933_runtimedeviceinstall.dll", "aadauthhelper.dll", "aadcloudap.dll", "aadjcsp.dll", "aadtb.dll", "aadwamextension.dll", "aarsvc.dll", "aboutsettingshandlers.dll", "abovelockapphost.dll", "accessibilitycpl.dll", "accountaccessor.dll", "accountsrt.dll", "acgenral.dll", "aclayers.dll", "acledit.dll", "aclui.dll", "acmigration.dll", "acpbackgroundmanagerpolicy.dll", "acppage.dll", "acproxy.dll", "acspecfc.dll", "actioncenter.dll", "actioncentercpl.dll", "actionqueue.dll", "activationclient.dll", "activationmanager.dll", "activeds.dll", "activesynccsp.dll", "activesyncprovider.dll", "actxprxy.dll", "acwinrt.dll", "acxtrnal.dll", "adal.dll", "adaptivecards.dll", "addressparser.dll", "adhapi.dll", "adhsvc.dll", "admtmpl.dll", "admwprox.dll", "adobepdf.dll", "adobepdfui.dll", "adprovider.dll", "adsldp.dll", "adsldpc.dll", "adsmsext.dll", "adsnt.dll", "adtschema.dll", "advancedemojids.dll", "advapi32.dll", "advapi32res.dll", "advpack.dll", "aeevts.dll", "aeinv.dll", "aemarebackup.dll", "aepic.dll", "agentactivationruntime.dll", "agentactivationruntimewindows.dll", "ahadmin.dll", "ajrouter.dll", "amsi.dll", "amsiproxy.dll", "amstream.dll", "analog.shell.broker.dll", "analogcommonproxystub.dll", "apds.dll", "aphostclient.dll", "aphostres.dll", "aphostservice.dll", "apisampling.dll", "apisethost.appexecutionalias.dll", "apisetschema.dll", "apmon.dll", "apmonui.dll", "appcontracts.dll", "appextension.dll", "apphelp.dll", "apphlpdm.dll", "appidapi.dll", "appidpolicyengineapi.dll", "appidsvc.dll", "appinfo.dll", "appinfoext.dll", "appinstallerprompt.desktop.dll", "applicationcontrolcsp.dll", "applicationframe.dll", "applicationtargetedfeaturedatabase.dll", "applistbackuplauncher.dll", "applockercsp.dll", "appmgmts.dll", "appmgr.dll", "appmon.dll", "appointmentactivation.dll", "appointmentapis.dll", "appraiser.dll", "appreadiness.dll", "apprepapi.dll", "appresolver.dll", "appsruprov.dll", "appverifui.dll", "appxalluserstore.dll", "appxapplicabilityblob.dll", "appxapplicabilityengine.dll", "appxdeploymentclient.dll", "appxdeploymentextensions.desktop.dll", "appxdeploymentextensions.onecore.dll", "appxdeploymentserver.dll", "appxpackaging.dll", "appxsip.dll", "appxstreamingdatasourceps.dll", "appxsysprep.dll", "apx01000.dll", "archiveint.dll", "asferror.dll", "aspnet_counters.dll", "aspperf.dll", "assignedaccessruntime.dll", "asycfilt.dll", "atl.dll", "atl100.dll", "atl110.dll", "atlthunk.dll", "atmlib.dll", "attestationwmiprovider.dll", "audioendpointbuilder.dll", "audioeng.dll", "audiohandlers.dll", "audiokse.dll", "audioresourceregistrar.dll", "audioses.dll", "audiosrv.dll", "audiosrvpolicymanager.dll", "auditcse.dll", "auditnativesnapin.dll", "auditpolcore.dll", "auditpolicygpinterop.dll", "auditpolmsg.dll", "authbroker.dll", "authbrokerui.dll", "authentication.dll", "authext.dll", "authfwcfg.dll", "authfwgp.dll", "authfwsnapin.dll", "authfwwizfwk.dll", "authhostproxy.dll", "authui.dll", "authz.dll", "automaticappsigninpolicy.dll", "autopilot.dll", "autopilotdiag.dll", "autoplay.dll", "autotimesvc.dll", "avicap32.dll", "avifil32.dll", "avrt.dll", "axinstsv.dll", "azroles.dll", "azroleui.dll", "azsqlext.dll", "backgroundmediapolicy.dll", "bamsettingsclient.dll", "barcodeprovisioningplugin.dll", "basecsp.dll", "basesrv.dll", "batmeter.dll", "bcastdvr.proxy.dll", "bcastdvrbroker.dll", "bcastdvrclient.dll", "bcastdvrcommon.dll", "bcastdvruserservice.dll", "bcd.dll", "bcdprov.dll", "bcdsrv.dll", "bcp47langs.dll", "bcp47mrm.dll", "bcrypt.dll", "bcryptprimitives.dll", "bdehdcfglib.dll", "bderepair.dll", "bdesvc.dll", "bdeui.dll", "bi.dll", "bidispl.dll", "bindfltapi.dll", "bingasds.dll", "bingfilterds.dll", "bingmaps.dll", "bingonlineservices.dll", "biocredprov.dll", "bisrv.dll", "bitlockercsp.dll", "bitsigd.dll", "bitsperf.dll", "bitsproxy.dll", "biwinrt.dll", "blbevents.dll", "blbres.dll", "blb_ps.dll", "bluetoothapis.dll", "bluetoothdesktophandlers.dll", "bluetoothopppushclient.dll", "bnmanager.dll", "bootmenuux.dll", "bootstr.dll", "bootsvc.dll", "bootux.dll", "bridgeres.dll", "brokerfiledialog.dll", "brokerlib.dll", "browcli.dll", "browser.dll", "browserbroker.dll", "browseui.dll", "btagservice.dll", "bthavctpsvc.dll", "bthavrcp.dll", "bthavrcpappsvc.dll", "bthci.dll", "bthmtpcontexthandler.dll", "bthpanapi.dll", "bthpancontexthandler.dll", "bthradiomedia.dll", "bthserv.dll", "bthtelemetry.dll", "btpanui.dll", "bwcontexthandler.dll", "c4d66f00-b6f0-4439-ac9b-c5ea13fe54d7_hyperv-computecore.dll", "cabapi.dll", "cabinet.dll", "cabview.dll", "callbuttons.dll", "callbuttons.proxystub.dll", "callhistoryclient.dll", "cameracaptureui.dll", "camext.dll", "capabilityaccesshandlers.dll", "capabilityaccessmanager.dll", "capabilityaccessmanagerclient.dll", "capauthz.dll", "capiprovider.dll", "capisp.dll", "captureservice.dll", "castingshellext.dll", "castlaunch.dll", "catsrv.dll", "catsrvps.dll", "catsrvut.dll", "cbdhsvc.dll", "cca.dll", "cdd.dll", "cdosys.dll", "cdp.dll", "cdprt.dll", "cdpsvc.dll", "cdpusersvc.dll", "cellulardatacapabilityhandler.dll", "cemapi.dll", "certca.dll", "certcli.dll", "certcredprovider.dll", "certenc.dll", "certenroll.dll", "certenrollui.dll", "certmgr.dll", "certpkicmdlet.dll", "certpoleng.dll", "certprop.dll", "cewmdm.dll", "cfgbkend.dll", "cfgmgr32.dll", "cfgspcellular.dll", "cfgsppolicy.dll", "cflapi.dll", "cfmifs.dll", "cfmifsproxy.dll", "chakra.dll", "chakradiag.dll", "chakrathunk.dll", "chartv.dll", "chatapis.dll", "chsstrokeds.dll", "chtbopomofods.dll", "chtcangjieds.dll", "chthkstrokeds.dll", "chtquickds.dll", "chxapds.dll", "chxdecoder.dll", "chxhapds.dll", "chxinputrouter.dll", "chxranker.dll", "chxreadingstringime.dll", "ci.dll", "cic.dll", "cimfs.dll", "circoinst.dll", "clbcatq.dll", "cldapi.dll", "cleanpccsp.dll", "clfsw32.dll", "cliconfg.dll", "clipboardserver.dll", "clipc.dll", "clipsvc.dll", "clipwinrt.dll", "cloudap.dll", "clouddesktopcsp.dll", "clouddomainjoinaug.dll", "clouddomainjoindatamodelserver.dll", "cloudexperiencehost.dll", "cloudexperiencehostbroker.dll", "cloudexperiencehostcommon.dll", "cloudexperiencehostredirection.dll", "cloudexperiencehostuser.dll", "cloudidwxhextension.dll", "cloudrecoverydownloadtool.dll", "cloudrestorelauncher.dll", "clrhost.dll", "clusapi.dll", "cmcfg32.dll", "cmdext.dll", "cmdial32.dll", "cmgrcspps.dll", "cmifw.dll", "cmintegrator.dll", "cmlua.dll", "cmpbk32.dll", "cmstplua.dll", "cmutil.dll", "cngcredui.dll", "cngkeyhelper.dll", "cngprovider.dll", "cnvfat.dll", "codeintegrityaggregator.dll", "cofiredm.dll", "colbact.dll", "colorui.dll", "combase.dll", "comcat.dll", "comctl32.dll", "comdlg32.dll", "coml2.dll", "compataggregator.dll", "composableshellproxystub.dll", "composerframework.dll", "comppkgsup.dll", "compstui.dll", "computecore.dll", "computelibeventlog.dll", "computenetwork.dll", "computestorage.dll", "comrepl.dll", "comres.dll", "comsnap.dll", "comsvcs.dll", "comuid.dll", "concrt140.dll", "concrt140d.dll", "configmanager2.dll", "configureexpandedstorage.dll", "conhostv1.dll", "connect.dll", "connectedaccountstate.dll", "consentexperiencecommon.dll", "consentux.dll", "consentuxclient.dll", "console.dll", "consolelogon.dll", "constraintindex.search.dll", "contactactivation.dll", "contactapis.dll", "contactharvesterds.dll", "container.dll", "containerdevicemanagement.dll", "contentdeliverymanager.utilities.dll", "controllib.dll", "coreaudiopolicymanagerext.dll", "coredpus.dll", "coreglobconfig.dll", "coremas.dll", "coremessaging.dll", "coremmres.dll", "coreprivacysettingsstore.dll", "coreshell.dll", "coreshellapi.dll", "coreshellextframework.dll", "coreuicomponents.dll", "correngine.dll", "courtesyengine.dll", "cpfilters.dll", "creddialogbroker.dll", "credentialenrollmentmanagerforuser.dll", "credprov2fahelper.dll", "credprovcommoncore.dll", "credprovdatamodel.dll", "credprovhelper.dll", "credprovhost.dll", "credprovs.dll", "credprovslegacy.dll", "credssp.dll", "credui.dll", "crypt32.dll", "cryptbase.dll", "cryptcatsvc.dll", "cryptdlg.dll", "cryptdll.dll", "cryptext.dll", "cryptnet.dll", "cryptngc.dll", "cryptowinrt.dll", "cryptsp.dll", "cryptsvc.dll", "crypttpmeksvc.dll", "cryptui.dll", "cryptuiwizard.dll", "cryptxml.dll", "cscapi.dll", "cscdll.dll", "cspcellularsettings.dll", "csplte.dll", "cspproxy.dll", "csrsrv.dll", "csystemeventsbrokerclient.dll", "cuzzapi.dll", "cxcredprov.dll", "cxhprovisioningserver.dll", "d2d1.dll", "d2d1debug3.dll", "d3d10.dll", "d3d10core.dll", "d3d10level9.dll", "d3d10ref.dll", "d3d10sdklayers.dll", "d3d10warp.dll", "d3d10_1.dll", "d3d10_1core.dll", "d3d11.dll", "d3d11on12.dll", "d3d11_3sdklayers.dll", "d3d12.dll", "d3d12core.dll", "d3d12sdklayers.dll", "d3d8thk.dll", "d3d9.dll", "d3d9on12.dll", "d3dcompiler_43.dll", "d3dcompiler_47.dll", "d3dcsx_43.dll", "d3dref9.dll", "d3dscache.dll", "d3dx10_43.dll", "d3dx11_43.dll", "d3dx9_30.dll", "d3dx9_43.dll", "d4d78066-e6db-44b7-b5cd-2eb82dce620c_hyperv-computelegacy.dll", "dab.dll", "dabapi.dll", "daconn.dll", "dafaspinfraprovider.dll", "dafbth.dll", "dafdnssd.dll", "dafdockingprovider.dll", "dafescl.dll", "dafgip.dll", "dafiot.dll", "dafipp.dll", "dafmcp.dll", "dafpos.dll", "dafprintprovider.dll", "dafupnp.dll", "dafwcn.dll", "dafwfdprovider.dll", "dafwiprov.dll", "dafwsd.dll", "damediamanager.dll", "damm.dll", "daotpcredentialprovider.dll", "das.dll", "dataclen.dll", "dataexchange.dll", "datusage.dll", "davclnt.dll", "davhlpr.dll", "davsyncprovider.dll", "daxexec.dll", "dbgcore.dll", "dbgeng.dll", "dbghelp.dll", "dbgmodel.dll", "dbnetlib.dll", "dbnmpntw.dll", "dciman32.dll", "dcntel.dll", "dcomp.dll", "dcsvc.dll", "ddaclsys.dll", "ddcclaimsapi.dll", "ddccomimplementationsdesktop.dll", "ddds.dll", "ddisplay.dll", "ddoiproxy.dll", "ddores.dll", "ddraw.dll", "ddrawex.dll", "declaredconfiguration.dll", "defaultdevicemanager.dll", "defaultprinterprovider.dll", "defragproxy.dll", "defragres.dll", "defragsvc.dll", "delegatorprovider.dll", "deploymentcsps.dll", "deskadp.dll", "deskmon.dll", "desktopshellappstatecontract.dll", "desktopshellext.dll", "desktopswitcherdatamodel.dll", "desktopview.internal.broker.dll", "desktopview.internal.broker.proxystub.dll", "devdispitemprovider.dll", "developeroptionssettingshandlers.dll", "devenum.dll", "deviceaccess.dll", "deviceassociation.dll", "devicecenter.dll", "devicecompanionappinstall.dll", "devicecredential.dll", "devicedirectoryclient.dll", "devicedisplaystatusmanager.dll", "devicedriverretrievalclient.dll", "deviceelementsource.dll", "deviceflows.datamodel.dll", "devicemetadataretrievalclient.dll", "devicengccredprov.dll", "devicepairing.dll", "devicepairingexperiencemem.dll", "devicepairingfolder.dll", "devicepairingproxy.dll", "devicereactivation.dll", "deviceregistration.dll", "devicesetupmanager.dll", "devicesetupmanagerapi.dll", "devicesetupstatusprovider.dll", "devicesflowbroker.dll", "devicesoftwareinstallationclient.dll", "deviceupdateagent.dll", "deviceuxres.dll", "devinv.dll", "devmgr.dll", "devobj.dll", "devpropmgr.dll", "devquerybroker.dll", "devrtl.dll", "dfdts.dll", "dfscli.dll", "dfshim.dll", "dfsshlex.dll", "dhcpcmonitor.dll", "dhcpcore.dll", "dhcpcore6.dll", "dhcpcsvc.dll", "dhcpcsvc6.dll", "dhcpsapi.dll", "dholographicdisplay.dll", "diagcpl.dll", "diagnosticdataquery.dll", "diagnosticdatasettings.dll", "diagnosticinvoker.dll", "diagnosticlogcsp.dll", "diagperf.dll", "diagsvc.dll", "diagtrack.dll", "dialclient.dll", "dialserver.dll", "dictationmanager.dll", "difxapi.dll", "dimsjob.dll", "dimsroam.dll", "dinput.dll", "dinput8.dll", "direct2ddesktop.dll", "directmanipulation.dll", "directml.debug.dll", "directml.dll", "directxdatabasehelper.dll", "discan.dll", "dismapi.dll", "dispbroker.desktop.dll", "dispbroker.dll", "dispex.dll", "display.dll", "displaymanager.dll", "dlnashext.dll", "dmalertlistener.proxystub.dll", "dmapisetextimpldesktop.dll", "dmappsres.dll", "dmcfgutils.dll", "dmcmnutils.dll", "dmcommandlineutils.dll", "dmcsps.dll", "dmdlgs.dll", "dmdskmgr.dll", "dmdskres.dll", "dmdskres2.dll", "dmenrollengine.dll", "dmenterprisediagnostics.dll", "dmintf.dll", "dmiso8601utils.dll", "dmloader.dll", "dmocx.dll", "dmoleaututils.dll", "dmprocessxmlfiltered.dll", "dmpushproxy.dll", "dmpushroutercore.dll", "dmrcdecoder.dll", "dmrserver.dll", "dmsynth.dll", "dmusic.dll", "dmutil.dll", "dmvdsitf.dll", "dmwappushsvc.dll", "dmwmicsp.dll", "dmxmlhelputils.dll", "dnsapi.dll", "dnscmmc.dll", "dnsext.dll", "dnshc.dll", "dnsrslvr.dll", "docking.virtualinput.dll", "dockinterface.proxystub.dll", "doclient.dll", "docprop.dll", "documentperformanceevents.dll", "dolbydecmft.dll", "domgmt.dll", "domiprov.dll", "dosettings.dll", "dosvc.dll", "dot3api.dll", "dot3cfg.dll", "dot3conn.dll", "dot3dlg.dll", "dot3gpclnt.dll", "dot3gpui.dll", "dot3hc.dll", "dot3mm.dll", "dot3msm.dll", "dot3svc.dll", "dot3ui.dll", "dpapi.dll", "dpapiprovider.dll", "dpapisrv.dll", "dplcsp.dll", "dpnaddr.dll", "dpnathlp.dll", "dpnet.dll", "dpnhpast.dll", "dpnhupnp.dll", "dpnlobby.dll", "dps.dll", "dpx.dll", "dragdropexperiencecommon.dll", "dragdropexperiencedataexchangedelegated.dll", "drprov.dll", "drt.dll", "drtprov.dll", "drttransport.dll", "drvsetup.dll", "drvstore.dll", "dsauth.dll", "dsccore.dll", "dsccoreconfprov.dll", "dsclient.dll", "dscproxy.dll", "dsctimer.dll", "dsdmo.dll", "dskquota.dll", "dskquoui.dll", "dsound.dll", "dsparse.dll", "dsprop.dll", "dsquery.dll", "dsreg.dll", "dsregtask.dll", "dsrole.dll", "dssec.dll", "dssenh.dll", "dssvc.dll", "dsui.dll", "dsuiext.dll", "dswave.dll", "dtsh.dll", "dtspipelineperf150.dll", "ducsps.dll", "dui70.dll", "duser.dll", "dusmapi.dll", "dusmsvc.dll", "dwmapi.dll", "dwmcore.dll", "dwmghost.dll", "dwminit.dll", "dwmredir.dll", "dwmscene.dll", "dwrite.dll", "dxcapturereplay.dll", "dxcore.dll", "dxdiagn.dll", "dxgi.dll", "dxgidebug.dll", "dxgwdi.dll", "dxilconv.dll", "dxmasf.dll", "dxp.dll", "dxpps.dll", "dxptasksync.dll", "dxtmsft.dll", "dxtoolsmonitor.dll", "dxtoolsofflineanalysis.dll", "dxtoolsreportgenerator.dll", "dxtoolsreporting.dll", "dxtrans.dll", "dxva2.dll", "dynamoapi.dll", "eamprogresshandler.dll", "eapp3hst.dll", "eappcfg.dll", "eappcfgui.dll", "eappgnui.dll", "eapphost.dll", "eappprxy.dll", "eapprovp.dll", "eapputil.dll", "eapsimextdesktop.dll", "eapsvc.dll", "eapteapauth.dll", "eapteapconfig.dll", "eapteapext.dll", "easconsent.dll", "easinvoker.proxystub.dll", "easpolicymanagerbrokerps.dll", "easwrt.dll", "edgeangle.dll", "edgecontent.dll", "edgehtml.dll", "edgeiso.dll", "edgemanager.dll", "edgeresetplugin.dll", "editbuffertesthook.dll", "editionupgradehelper.dll", "editionupgrademanagerobj.dll", "edpauditapi.dll", "edpcsp.dll", "edptask.dll", "edputil.dll", "eeprov.dll", "eeutil.dll", "efsadu.dll", "efscore.dll", "efsext.dll", "efslsaext.dll", "efssvc.dll", "efsutil.dll", "efswrt.dll", "ehstorapi.dll", "ehstorpwdmgr.dll", "ehstorshell.dll", "elevocdapo.dll", "elevocdnsengine.dll", "elevocgna.dll", "elevockwsapo.dll", "elevocseengine.dll", "elevocuapo.dll", "elevocunsengine.dll", "elevoc_kws_engine.dll", "elevoc_speech_engine.dll", "elevoc_teams_aec.dll", "elevoc_voice_separation.dll", "els.dll", "elscore.dll", "elshyph.dll", "elslad.dll", "elstrans.dll", "emailapis.dll", "embeddedmodesvc.dll", "embeddedmodesvcapi.dll", "emojids.dll", "encapi.dll", "enclave_ioc.signed.dll", "enclave_ssl.signed.dll", "energy.dll", "energyprov.dll", "energytask.dll", "enrollmentapi.dll", "enterpriseapncsp.dll", "enterpriseappmgmtclient.dll", "enterpriseappmgmtsvc.dll", "enterprisecsps.dll", "enterprisedesktopappmgmtcsp.dll", "enterpriseetw.dll", "enterprisemodernappmgmtcsp.dll", "enterpriseresourcemanager.dll", "eqossnap.dll", "errordetails.dll", "errordetailscore.dll", "es.dll", "esclprotocol.dll", "esclscan.dll", "esclwiadriver.dll", "esdsip.dll", "esent.dll", "esentprf.dll", "esevss.dll", "eshims.dll", "ethernetmediamanager.dll", "etwcoreuicomponentsresources.dll", "etweseproviderresources.dll", "etwrundown.dll", "euiccscsp.dll", "eventaggregation.dll", "eventcls.dll", "evr.dll", "execmodelclient.dll", "execmodelproxy.dll", "explorerframe.dll", "exsmime.dll", "extrasxmlparser.dll", "f1db7d81-95be-4911-935a-8ab71629112a_hyperv-isolatedvm.dll", "f3ahvoas.dll", "f989b52d-f928-44a3-9bf1-bf0c1da6a0d6_hyperv-devicevirtualization.dll", "facecredentialprovider.dll", "face_beauty_dll_x64.dll", "facilitator.dll", "family.authentication.dll", "family.cache.dll", "family.client.dll", "family.syncengine.dll", "familysafetyext.dll", "faultrep.dll", "faxprinterinstaller.dll", "fcon.dll", "fcstdthumbnail.dll", "fdbth.dll", "fdbthproxy.dll", "fddevquery.dll", "fde.dll", "fdeploy.dll", "fdphost.dll", "fdpnp.dll", "fdprint.dll", "fdproxy.dll", "fdrespub.dll", "fdssdp.dll", "fdwcn.dll", "fdwnet.dll", "fdwsd.dll", "feclient.dll", "ffbroker.dll", "fhcat.dll", "fhcfg.dll", "fhcleanup.dll", "fhcpl.dll", "fhengine.dll", "fhevents.dll", "fhsettingsprovider.dll", "fhshl.dll", "fhsrchapi.dll", "fhsrchph.dll", "fhsvc.dll", "fhsvcctl.dll", "fhtask.dll", "fhuxadapter.dll", "fhuxapi.dll", "fhuxcommon.dll", "fhuxgraphics.dll", "fhuxpresentation.dll", "fidocredprov.dll", "fileappxstreamingdatasource.dll", "filemgmt.dll", "filterds.dll", "findnetprinters.dll", "fingerprintcredential.dll", "firewallapi.dll", "firewallcontrolpanel.dll", "firewallux.dll", "firmwareattestationserverproxystub.dll", "flightsettings.dll", "fltlib.dll", "fluencyds.dll", "fmapi.dll", "fmifs.dll", "fmmp.dll", "fms.dll", "fntcache.dll", "fontext.dll", "fontglyphanimator.dll", "fontgroupsoverride.dll", "fontprovider.dll", "fontsub.dll", "fphc.dll", "framedyn.dll", "framedynos.dll", "frameserver.dll", "frameserverclient.dll", "frameservermonitor.dll", "frameservermonitorclient.dll", "frprov.dll", "fsnvsdevicesource.dll", "fssres.dll", "fsutilext.dll", "fthsvc.dll", "fundisc.dll", "fveapi.dll", "fveapibase.dll", "fvecerts.dll", "fvecpl.dll", "fveskybackup.dll", "fveui.dll", "fvewiz.dll", "fvsdk_x64.dll", "fwbase.dll", "fwcfg.dll", "fwmdmcsp.dll", "fwpolicyiomgr.dll", "fwremotesvr.dll", "fxsapi.dll", "fxscom.dll", "fxscomex.dll", "fxscompose.dll", "fxscomposeres.dll", "fxsevent.dll", "fxsmon.dll", "fxsresm.dll", "fxsroute.dll", "fxsst.dll", "fxst30.dll", "fxstiff.dll", "fxsutility.dll", "gamebarpresencewriter.proxy.dll", "gamechatoverlayext.dll", "gamechattranscription.dll", "gameconfighelper.dll", "gameinput.dll", "gameinputinbox.dll", "gameinputredist.dll", "gamelaunchhelper.dll", "gamemode.dll", "gamepanelexternalhook.dll", "gameplatformservices.dll", "gamestreamingext.dll", "gameux.dll", "gamingservicesproxy_4.dll", "gamingtcui.dll", "gamingtcuihelpers.dll", "gcdef.dll", "gdi32.dll", "gdi32full.dll", "gdiplus.dll", "generaltel.dll", "geocommon.dll", "geolocation.dll", "getuname.dll", "glmf32.dll", "globinputhost.dll", "glu32.dll", "gmsaclient.dll", "gna.dll", "gnaplugin.dll", "gpapi.dll", "gpcsewrappercsp.dll", "gpedit.dll", "gpprefcl.dll", "gpprnext.dll", "gpscript.dll", "gpsvc.dll", "gptext.dll", "gpupvdev.dll", "graphicscapture.dll", "graphicsperfsvc.dll", "groupinghc.dll", "hadrres.dll", "hal.dll", "halextintclpiodma.dll", "halextintcpsedma.dll", "halextpl080.dll", "hanjads.dll", "hascsp.dll", "hashtagds.dll", "haspsrm_win64.dll", "hbaapi.dll", "hcproviders.dll", "hdcphandler.dll", "heatcore.dll", "helppaneproxy.dll", "hgattest.dll", "hgclientservice.dll", "hgclientserviceps.dll", "hgcpl.dll", "hgsclientplugin.dll", "hgsclientwmi.dll", "hhsetup.dll", "hid.dll", "hidcfu.dll", "hidserv.dll", "hlink.dll", "hmkd.dll", "hnetcfg.dll", "hnetcfgclient.dll", "hnetmon.dll", "hnsproxy.dll", "hologramcompositor.dll", "hologramworld.dll", "holographicextensions.dll", "holographicruntimes.dll", "holoshellruntime.dll", "holoshextensions.dll", "holosi.pcshell.dll", "hostguardianserviceclientresources.dll", "hostnetsvc.dll", "hotplug.dll", "hrtfapo.dll", "hrtfdspcpu.dll", "hspapi.dll", "hspfw.dll", "httpapi.dll", "httpprxc.dll", "httpprxm.dll", "httpprxp.dll", "httpsdatasource.dll", "htui.dll", "hvhostsvc.dll", "hvloader.dll", "hvsocket.dll", "hwreqchk.dll", "hydrogen.dll", "hypervsysprepprovider.dll", "ia2comproxy.dll", "ias.dll", "iasacct.dll", "iasads.dll", "iasdatastore.dll", "iashlpr.dll", "iasmigplugin.dll", "iasnap.dll", "iaspolcy.dll", "iasrad.dll", "iasrecst.dll", "iassam.dll", "iassdo.dll", "iassvcs.dll", "icfupgd.dll", "icm32.dll", "icmp.dll", "icmui.dll", "iconcodecservice.dll", "icsigd.dll", "icsvc.dll", "icsvcext.dll", "icsvcvss.dll", "icu.dll", "icuin.dll", "icuuc.dll", "idctrls.dll", "idstore.dll", "ieadvpack.dll", "ieapfltr.dll", "iedkcs32.dll", "ieframe.dll", "iemigplugin.dll", "iepeers.dll", "ieproxy.dll", "ieproxydesktop.dll", "iernonce.dll", "iertutil.dll", "iesetup.dll", "iesysprep.dll", "ieui.dll", "ifmon.dll", "ifsutil.dll", "ifsutilx.dll", "igddiag.dll", "ihds.dll", "iisrstap.dll", "iisrtl.dll", "imagehlp.dll", "imageres.dll", "imagesp1.dll", "imapi.dll", "imapi2.dll", "imapi2fs.dll", "ime_textinputhelpers.dll", "imgutil.dll", "imm32.dll", "implatsetup.dll", "indexeddblegacy.dll", "inetcomm.dll", "inetmib1.dll", "inetpp.dll", "inetppui.dll", "inetres.dll", "inference_engine.dll", "inference_engine_c_api.dll", "inference_engine_legacy.dll", "inference_engine_transformations.dll", "inked.dll", "inkobjcore.dll", "inproclogger.dll", "input.dll", "inputcloudstore.dll", "inputcontroller.dll", "inputhost.dll", "inputinjectionbroker.dll", "inputlocalemanager.dll", "inputservice.dll", "inputswitch.dll", "inputviewexperience.dll", "inseng.dll", "installservice.dll", "installservicetasks.dll", "intelligentpwdlesstask.dll", "intel_gfx_api-x64.dll", "internetmail.dll", "internetmailcsp.dll", "invagent.dll", "inventorysvc.dll", "iologmsg.dll", "ipeloggingdictationhelper.dll", "iphlpsvc.dll", "ipnathlp.dll", "ipnathlpclient.dll", "ippcommon.dll", "ippcommonproxy.dll", "iprtprio.dll", "iprtrmgr.dll", "ipsecsnp.dll", "ipsmsnap.dll", "ipxlatcfg.dll", "iri.dll", "iscsicpl.dll", "iscsidsc.dll", "iscsied.dll", "iscsiexe.dll", "iscsilog.dll", "iscsium.dll", "iscsiwmi.dll", "iscsiwmiv2.dll", "ism.dll", "itircl.dll", "itss.dll", "iuilp.dll", "iumbase.dll", "iumcrypt.dll", "iumdll.dll", "iumsdk.dll", "iyuv_32.dll", "javascriptcollectionagent.dll", "jhi64.dll", "joinproviderol.dll", "joinutil.dll", "jpmapcontrol.dll", "jpndecoder.dll", "jpninputrouter.dll", "jpnranker.dll", "jpnserviceds.dll", "jscript.dll", "jscript9.dll", "jscript9diag.dll", "jscript9legacy.dll", "jsproxy.dll", "kbd101.dll", "kbd101a.dll", "kbd101b.dll", "kbd101c.dll", "kbd103.dll", "kbd106.dll", "kbd106n.dll", "kbdarmph.dll", "kbdarmty.dll", "kbdax2.dll", "kbdfar.dll", "kbdgeoer.dll", "kbdgeome.dll", "kbdgeooa.dll", "kbdgeoqw.dll", "kbdhebl3.dll", "kbdibm02.dll", "kbdlisub.dll", "kbdlisus.dll", "kbdlk41a.dll", "kbdnec.dll", "kbdnec95.dll", "kbdnecat.dll", "kbdnecnt.dll", "kbdnko.dll", "kbdphags.dll", "kd.dll", "kdcom.dll", "kdcpw.dll", "kdhvcom.dll", "kdnet.dll", "kdnet_uart16550.dll", "kdscli.dll", "kdstub.dll", "kdusb.dll", "kd_02_10df.dll", "kd_02_10ec.dll", "kd_02_1137.dll", "kd_02_14e4.dll", "kd_02_15b3.dll", "kd_02_1969.dll", "kd_02_19a2.dll", "kd_02_1af4.dll", "kd_02_8086.dll", "kd_07_1415.dll", "kd_0c_8086.dll", "keepaliveprovider.dll", "kerbclientshared.dll", "kerberos.dll", "kernel.appcore.dll", "kernel32.dll", "kernelbase.dll", "keycredmgr.dll", "keyiso.dll", "keymgr.dll", "keyworddetectormsftsidadapter.dll", "knobscore.dll", "knobscsp.dll", "ksuser.dll", "ktmw32.dll", "l2gpstore.dll", "l2nacp.dll", "l2sechc.dll", "langcleanupsysprepaction.dll", "languagecomponentsinstaller.dll", "languageoverlayserver.dll", "languageoverlayutil.dll", "languagepackdiskcleanup.dll", "languagepackmanagementcsp.dll", "laps.dll", "lapscsp.dll", "legacynetux.dll", "legacysystemsettings.dll", "lfsvc.dll", "libcrypto.dll", "libmfxhw64.dll", "libomp140.x86_64.dll", "libomp140d.x86_64.dll", "libvpl.dll", "licensemanager.dll", "licensemanagerapi.dll", "licensemanagersvc.dll", "licenseprotection.dll", "licensingcsp.dll", "licensingdiagspp.dll", "licensingwinrt.dll", "licmgr10.dll", "linkinfo.dll", "lltdapi.dll", "lltdres.dll", "lltdsvc.dll", "lmhsvc.dll", "loadperf.dll", "localsec.dll", "localspl.dll", "localui.dll", "locationapi.dll", "locationframework.dll", "locationframeworkinternalps.dll", "locationframeworkps.dll", "locationwinpalmisc.dll", "lockappbroker.dll", "lockcontroller.dll", "lockhostingframework.dll", "lockscreencontent.dll", "lockscreencontenthost.dll", "lockscreendata.dll", "loghours.dll", "logoncli.dll", "logoncontroller.dll", "lpasvc.dll", "lpk.dll", "lpksetupproxyserv.dll", "lsaadt.dll", "lsasrv.dll", "lsm.dll", "lsmproxy.dll", "luiapi.dll", "lxutil.dll", "lz32.dll", "magnification.dll", "maintenanceui.dll", "manageci.dll", "mapconfiguration.dll", "mapcontrolcore.dll", "mapcontrolstringsres.dll", "mapgeocoder.dll", "mapi32.dll", "mapistub.dll", "maprouter.dll", "mapsbtsvc.dll", "mapsbtsvcproxy.dll", "mapscsp.dll", "mapsstore.dll", "mapstoasttask.dll", "mapsupdatetask.dll", "mbaeapi.dll", "mbaeapipublic.dll", "mbmediamanager.dll", "mbsmsapi.dll", "mbussdapi.dll", "mccsengineshared.dll", "mccspal.dll", "mciavi32.dll", "mcicda.dll", "mciqtz32.dll", "mciseq.dll", "mciwave.dll", "mcpmanagementproxy.dll", "mcpmanagementservice.dll", "mcrecvsrc.dll", "mcupdate_authenticamd.dll", "mcupdate_genuineintel.dll", "mdmcommon.dll", "mdmdiagnostics.dll", "mdminst.dll", "mdmlocalmanagement.dll", "mdmmigrator.dll", "mdmpostprocessevaluator.dll", "mdmregistration.dll", "mediafoundation.defaultperceptionprovider.dll", "mediafoundationaggregator.dll", "memorydiagnostic.dll", "messagingdatamodel2.dll", "messagingservice.dll", "mf.dll", "mf3216.dll", "mfaacenc.dll", "mfasfsrcsnk.dll", "mfaudiocnv.dll", "mfc100.dll", "mfc100chs.dll", "mfc100cht.dll", "mfc100deu.dll", "mfc100enu.dll", "mfc100esn.dll", "mfc100fra.dll", "mfc100ita.dll", "mfc100jpn.dll", "mfc100kor.dll", "mfc100rus.dll", "mfc100u.dll", "mfc110.dll", "mfc110chs.dll", "mfc110cht.dll", "mfc110deu.dll", "mfc110enu.dll", "mfc110esn.dll", "mfc110fra.dll", "mfc110ita.dll", "mfc110jpn.dll", "mfc110kor.dll", "mfc110rus.dll", "mfc110u.dll", "mfc120.dll", "mfc120chs.dll", "mfc120cht.dll", "mfc120deu.dll", "mfc120enu.dll", "mfc120esn.dll", "mfc120fra.dll", "mfc120ita.dll", "mfc120jpn.dll", "mfc120kor.dll", "mfc120rus.dll", "mfc120u.dll", "mfc140.dll", "mfc140chs.dll", "mfc140cht.dll", "mfc140d.dll", "mfc140deu.dll", "mfc140enu.dll", "mfc140esn.dll", "mfc140fra.dll", "mfc140ita.dll", "mfc140jpn.dll", "mfc140kor.dll", "mfc140rus.dll", "mfc140u.dll", "mfc140ud.dll", "mfc42.dll", "mfc42u.dll", "mfcaptureengine.dll", "mfcm100.dll", "mfcm100u.dll", "mfcm110.dll", "mfcm110u.dll", "mfcm120.dll", "mfcm120u.dll", "mfcm140.dll", "mfcm140d.dll", "mfcm140u.dll", "mfcm140ud.dll", "mfcore.dll", "mfcsubs.dll", "mfds.dll", "mfdvdec.dll", "mferror.dll", "mfh263enc.dll", "mfh264enc.dll", "mfksproxy.dll", "mfmediaengine.dll", "mfmjpegdec.dll", "mfmkvsrcsnk.dll", "mfmp4srcsnk.dll", "mfmpeg2srcsnk.dll", "mfnetcore.dll", "mfnetsrc.dll", "mfperfhelper.dll", "mfplat.dll", "mfplay.dll", "mfps.dll", "mfreadwrite.dll", "mfsensorgroup.dll", "mfsrcsnk.dll", "mfsvr.dll", "mftranscode.dll", "mfvdsp.dll", "mfvfw.dll", "mfxplugin64_hw.dll", "mgmtapi.dll", "mgmtrefreshcredprov.dll", "mi.dll", "mibincodec.dll", "microsoft-windows-appmodelexecevents.dll", "microsoft-windows-battery-events.dll", "microsoft-windows-hal-events.dll", "microsoft-windows-internal-shell-nearshareexperience.dll", "microsoft-windows-kernel-cc-events.dll", "microsoft-windows-kernel-pnp-events.dll", "microsoft-windows-kernel-power-events.dll", "microsoft-windows-kernel-processor-power-events.dll", "microsoft-windows-mapcontrols.dll", "microsoft-windows-moshost.dll", "microsoft-windows-pdc.dll", "microsoft-windows-power-cad-events.dll", "microsoft-windows-processor-aggregator-events.dll", "microsoft-windows-sleepstudy-events.dll", "microsoft-windows-storage-tiering-events.dll", "microsoft-windows-system-events.dll", "microsoft-windowsphone-semanagementprovider.dll", "microsoft.bluetooth.audio.dll", "microsoft.bluetooth.proxy.dll", "microsoft.bluetooth.service.dll", "microsoft.bluetooth.userservice.dll", "microsoft.graphics.display.displayenhancementservice.dll", "microsoft.internal.frameworkudk.system.dll", "microsoft.localuserimageprovider.dll", "microsoft.management.infrastructure.native.unmanaged.dll", "microsoft.windows.storage.core.dll", "microsoft.windows.storage.storagebuscache.dll", "microsoftaccount.tokenprovider.core.dll", "microsoftaccountcloudap.dll", "microsoftaccountextension.dll", "microsoftaccounttokenprovider.dll", "microsoftaccountwamextension.dll", "midimap.dll", "migisol.dll", "miguiresource.dll", "mimefilt.dll", "mimofcodec.dll", "minstoreevents.dll", "miracastinputmgr.dll", "miracastreceiver.dll", "miracastreceiverext.dll", "mirrordrvcompat.dll", "mispace.dll", "mitigationclient.dll", "mitigationconfiguration.dll", "miutils.dll", "mixedreality.broker.dll", "mixedrealitycapture.pipeline.dll", "mixedrealitycapture.proxystub.dll", "mixedrealityruntime.dll", "mlang.dll", "mmcbase.dll", "mmcndmgr.dll", "mmcshext.dll", "mmdevapi.dll", "mmgaclient.dll", "mmgaproxystub.dll", "mmres.dll", "mobilenetworking.dll", "modemui.dll", "modernexecserver.dll", "moricons.dll", "moshost.dll", "moshostclient.dll", "moshostcore.dll", "mosstorage.dll", "mpeval.dll", "mpr.dll", "mprapi.dll", "mprddm.dll", "mprdim.dll", "mprext.dll", "mprmsg.dll", "mpssvc.dll", "mpunits.dll", "mrmcorer.dll", "mrmdeploy.dll", "mrmindexer.dll", "mrt100.dll", "mrt_map.dll", "ms3dthumbnailprovider.dll", "msaatext.dll", "msacm32.dll", "msafd.dll", "msajapi.dll", "msalacdecoder.dll", "msalacencoder.dll", "msamrnbdecoder.dll", "msamrnbencoder.dll", "msamrnbsink.dll", "msamrnbsource.dll", "msapofxproxy.dll", "msaprofilenotificationhandler.dll", "msasn1.dll", "msauddecmft.dll", "msaudite.dll", "msauserext.dll", "mscandui.dll", "mscat32.dll", "msclmd.dll", "mscms.dll", "mscoree.dll", "mscorier.dll", "mscories.dll", "msctf.dll", "msctfmonitor.dll", "msctfp.dll", "msctfui.dll", "msctfuimanager.dll", "msdadiag.dll", "msdart.dll", "msdelta.dll", "msdmo.dll", "msdrm.dll", "msdtckrm.dll", "msdtclog.dll", "msdtcprx.dll", "msdtcspoffln.dll", "msdtctm.dll", "msdtcuiu.dll", "msdtcvsp1res.dll", "msfeeds.dll", "msfeedsbs.dll", "msflacdecoder.dll", "msflacencoder.dll", "msftedit.dll", "msftoemdlligneous.dll", "msheif.dll", "mshtml.dll", "mshtmldac.dll", "mshtmled.dll", "mshtmler.dll", "msi.dll", "msicofire.dll", "msidcrl40.dll", "msident.dll", "msidle.dll", "msidntld.dll", "msieftp.dll", "msihnd.dll", "msiltcfg.dll", "msimg32.dll", "msimsg.dll", "msimtf.dll", "msisip.dll", "msiso.dll", "msiwer.dll", "msixdatasourceextensionps.dll", "mskeyprotcli.dll", "mskeyprotect.dll", "msls31.dll", "msmpeg2adec.dll", "msmpeg2vdec.dll", "msobjs.dll", "msodbcdiag11.dll", "msodbcdiag17.dll", "msodbcsql11.dll", "msodbcsql17.dll", "msoert2.dll", "msoledbsql.dll", "msopusdecoder.dll", "mspatcha.dll", "mspatchc.dll", "msphotography.dll", "msports.dll", "msprivs.dll", "msrahc.dll", "msrating.dll", "msrawimage.dll", "msrdc.dll", "msrdpwebaccess.dll", "msrle32.dll", "msscntrs.dll", "mssign32.dll", "mssip32.dll", "mssitlb.dll", "msspellcheckingfacility.dll", "mssph.dll", "mssprxy.dll", "mssrch.dll", "mssvp.dll", "mstask.dll", "mstextprediction.dll", "mstscax.dll", "msutb.dll", "msv1_0.dll", "msvcirt.dll", "msvcp100.dll", "msvcp110.dll", "msvcp110_win.dll", "msvcp120.dll", "msvcp120_clr0400.dll", "msvcp140.dll", "msvcp140d.dll", "msvcp140d_atomic_wait.dll", "msvcp140d_codecvt_ids.dll", "msvcp140_1.dll", "msvcp140_1d.dll", "msvcp140_2.dll", "msvcp140_2d.dll", "msvcp140_atomic_wait.dll", "msvcp140_clr0400.dll", "msvcp140_codecvt_ids.dll", "msvcp60.dll", "msvcp_win.dll", "msvcr100.dll", "msvcr100_clr0400.dll", "msvcr110.dll", "msvcr120.dll", "msvcr120_clr0400.dll", "msvcrt.dll", "msvfw32.dll", "msvidc32.dll", "msvidctl.dll", "msvideodsp.dll", "msvp9dec.dll", "msvproc.dll", "msvpxenc.dll", "mswb7.dll", "mswb70011.dll", "mswb70804.dll", "mswebp.dll", "mswmdm.dll", "mswsock.dll", "msxml3.dll", "msxml3r.dll", "msxml6.dll", "msxml6r.dll", "msyuv.dll", "mtcmodel.dll", "mtf.dll", "mtfappserviceds.dll", "mtfdecoder.dll", "mtffuzzyds.dll", "mtfserver.dll", "mtfspellcheckds.dll", "mtxclu.dll", "mtxdm.dll", "mtxex.dll", "mtxoci.dll", "muifontsetup.dll", "muilanguagecleanup.dll", "museuxdocked.dll", "musupdatehandlers.dll", "mycomput.dll", "mydocs.dll", "nahimicapo3configuratordaemonmodule.dll", "nahimicapo4.dll", "nahimicapo4api.dll", "nahimicapo4configuratordaemonmodule.dll", "nahimicapo4expertapi.dll", "nahimicpnpapo4configuratordaemonmodule.dll", "napinsp.dll", "naturalauth.dll", "naturalauthclient.dll", "naturallanguage6.dll", "navshutdown.dll", "ncaapi.dll", "ncasvc.dll", "ncbservice.dll", "ncdautosetup.dll", "ncdprop.dll", "nci.dll", "ncobjapi.dll", "ncrypt.dll", "ncryptprov.dll", "ncryptsslp.dll", "ncsi.dll", "ncuprov.dll", "nddeapi.dll", "ndfapi.dll", "ndfetw.dll", "ndfhcdiscovery.dll", "ndishc.dll", "ndproxystub.dll", "nduprov.dll", "negoexts.dll", "netapi32.dll", "netbios.dll", "netcenter.dll", "netcfgx.dll", "netcorehc.dll", "netdiagfx.dll", "netdriverinstall.dll", "netevent.dll", "netfxperf.dll", "neth.dll", "netid.dll", "netiohlp.dll", "netjoin.dll", "netlogon.dll", "netman.dll", "netmgmtif.dll", "netmsg.dll", "netplwiz.dll", "netprofm.dll", "netprofmsvc.dll", "netprovfw.dll", "netprovisionsp.dll", "netsetupapi.dll", "netsetupengine.dll", "netsetupshim.dll", "netsetupsvc.dll", "netshell.dll", "nettrace.dll", "netutils.dll", "networkbindingenginemigplugin.dll", "networkcollectionagent.dll", "networkdesktopsettings.dll", "networkexplorer.dll", "networkhelper.dll", "networkicon.dll", "networkitemfactory.dll", "networkmobilesettings.dll", "networkproxycsp.dll", "networkqospolicycsp.dll", "networkuxbroker.dll", "newdev.dll", "nfcprovisioningplugin.dll", "nfcradiomedia.dll", "ngccredprov.dll", "ngcctnr.dll", "ngcctnrgidshandler.dll", "ngcctnrsvc.dll", "ngcisoctnr.dll", "ngckeyenum.dll", "ngcksp.dll", "ngclocal.dll", "ngcpopkeysrv.dll", "ngcprocsp.dll", "ngcrecovery.dll", "ngcsvc.dll", "ngctasks.dll", "ngcutils.dll", "ngraph.dll", "nhnotifsys.dll", "ninput.dll", "nl7data0011.dll", "nl7data0804.dll", "nl7lexicons0011.dll", "nl7lexicons0804.dll", "nl7models0011.dll", "nl7models0804.dll", "nlaapi.dll", "nlahc.dll", "nlansp_c.dll", "nlhtml.dll", "nlmgp.dll", "nlmproxy.dll", "nlmsprep.dll", "nlsbres.dll", "nlsdata0000.dll", "nlsdata0009.dll", "nlsdl.dll", "nlslexicons0009.dll", "nmadirect.dll", "noise.dll", "nonarpinv.dll", "normaliz.dll", "notificationcontroller.dll", "notificationcontrollerps.dll", "notificationintelligenceplatform.dll", "notificationplatformcomponent.dll", "npmproxy.dll", "npsm.dll", "npsmdesktopprovider.dll", "nrpsrv.dll", "nrtapi.dll", "nshhttp.dll", "nshipsec.dll", "nshwfp.dll", "nsi.dll", "nsisvc.dll", "ntasn1.dll", "ntdll.dll", "ntdsapi.dll", "ntfsres.dll", "ntlanman.dll", "ntlanui2.dll", "ntlmshared.dll", "ntmarta.dll", "ntprint.dll", "ntshrui.dll", "ntvdm64.dll", "nvagent.dll", "nvapi64.dll", "nvaudcap64v.dll", "nvcpl.dll", "nvcuda.dll", "nvcudadebugger.dll", "nvcuvid.dll", "nvencodeapi64.dll", "nvfbc64.dll", "nvifr64.dll", "nvml.dll", "nvofapi64.dll", "nvrtmpstreamer64.dll", "nvspcap64.dll", "objsel.dll", "occache.dll", "ocsetapi.dll", "odbc32.dll", "odbcbcp.dll", "odbcconf.dll", "odbccp32.dll", "odbccr32.dll", "odbccu32.dll", "odbcint.dll", "odbctrac.dll", "oemdefaultassociations.dll", "oemlicense.dll", "offfilt.dll", "officecsp.dll", "offlinelsa.dll", "offlinesam.dll", "offreg.dll", "ole32.dll", "oleacc.dll", "oleacchooks.dll", "oleaccrc.dll", "oleaut32.dll", "oledlg.dll", "oleprn.dll", "omadmagent.dll", "omadmapi.dll", "ondemandbrokerclient.dll", "ondemandconnroutehelper.dll", "onebackuphandler.dll", "onecorecommonproxystub.dll", "onecoreuapcommonproxystub.dll", "onesettingsclient.dll", "onex.dll", "onexui.dll", "onnxruntime.dll", "opcservices.dll", "opencl.dll", "opengl32.dll", "ortcengine.dll", "osbaseln.dll", "osksupport.dll", "osuninst.dll", "p2p.dll", "p2pgraph.dll", "p2pnetsh.dll", "p2psvc.dll", "p9np.dll", "p9rdrservice.dll", "packager.dll", "packagestatechangehandler.dll", "panmap.dll", "passwordenrollmentmanager.dll", "pautoenr.dll", "payloadrestrictions.dll", "paymentmediatorserviceproxy.dll", "pcacli.dll", "pcadm.dll", "pcaevts.dll", "pcasvc.dll", "pcaui.dll", "pcpksp.dll", "pcshellcommonproxystub.dll", "pcsvdevice.dll", "pcwum.dll", "pcwutl.dll", "pdh.dll", "pdhui.dll", "penservice.dll", "peopleapis.dll", "peopleband.dll", "perceptiondevice.dll", "perceptionsimulation.proxystubs.dll", "perceptionsimulationmanager.dll", "perf-mssql$sqlexpress-sqlctr15.0.2000.5.dll", "perf-mssql15.sqlexpress-sqlagtctr.dll", "perfdisk.dll", "perfnet.dll", "perfos.dll", "perfproc.dll", "perfts.dll", "perf_gputiming.dll", "personalizationcsp.dll", "pfclient.dll", "phonecallhistoryapis.dll", "phoneom.dll", "phoneplatformabstraction.dll", "phoneproviders.dll", "phoneservice.dll", "phoneserviceres.dll", "phoneutil.dll", "phoneutilres.dll", "photometadatahandler.dll", "photowiz.dll", "pickerplatform.dll", "pid.dll", "pidgenx.dll", "pifmgr.dll", "pimindexmaintenance.dll", "pimindexmaintenanceclient.dll", "pimstore.dll", "pinenrollmenthelper.dll", "pkeyhelper.dll", "pktmonapi.dll", "pku2u.dll", "pla.dll", "playlistfolder.dll", "playsndsrv.dll", "playtodevice.dll", "playtomanager.dll", "playtomenu.dll", "playtoreceiver.dll", "playtostatusprovider.dll", "ploptin.dll", "pngfilt.dll", "pnidui.dll", "pnpclean.dll", "pnpdiag.dll", "pnppolicy.dll", "pnpts.dll", "pnpui.dll", "pnpxassoc.dll", "pnpxassocprx.dll", "pnrpauto.dll", "pnrphc.dll", "pnrpnsp.dll", "pnrpsvc.dll", "policymanager.dll", "policymanagerprecheck.dll", "polstore.dll", "portabledeviceapi.dll", "portabledeviceclassextension.dll", "portabledeviceconnectapi.dll", "portabledevicestatus.dll", "portabledevicesyncprovider.dll", "portabledevicetypes.dll", "portabledevicewiacompat.dll", "posetup.dll", "posyncservices.dll", "pots.dll", "powercpl.dll", "powrprof.dll", "prauthproviders.dll", "presentationcffrasterizernative_v0300.dll", "presentationhostproxy.dll", "presentationnative_v0300.dll", "prflbmsg.dll", "print.printsupport.source.dll", "print.workflow.source.dll", "printercleanuptask.dll", "printfilterpipelineprxy.dll", "printisolationproxy.dll", "printnotification.dll", "printplatformconfig.dll", "printticketvalidation.dll", "printui.dll", "printworkflowservice.dll", "printwsdahost.dll", "prm0009.dll", "prm0019.dll", "prncache.dll", "prnfldr.dll", "prnntfy.dll", "prntvpt.dll", "productenumerator.dll", "profapi.dll", "profext.dll", "profprov.dll", "profsvc.dll", "profsvcext.dll", "propsys.dll", "provcore.dll", "provdatastore.dll", "provdiagnostics.dll", "provengine.dll", "provhandlers.dll", "provisioningcommandscsp.dll", "provisioningcsp.dll", "provisioninghandlers.dll", "provmigrate.dll", "provops.dll", "provpackageapidll.dll", "provplatformdesktop.dll", "provplugineng.dll", "provsysprep.dll", "provthrd.dll", "proximitycommon.dll", "proximitycommonpal.dll", "proximityrtapipal.dll", "proximityservice.dll", "proximityservicepal.dll", "prvdmofcomp.dll", "prxyqry.dll", "psapi.dll", "psisdecd.dll", "psmodulediscoveryprovider.dll", "psmserviceexthost.dll", "psmsrv.dll", "pstask.dll", "pstorec.dll", "ptpprov.dll", "puiapi.dll", "puiobj.dll", "pushtoinstall.dll", "pwdlessaggregator.dll", "pwlauncher.dll", "pwrshplugin.dll", "pwrshsip.dll", "pwsso.dll", "qasf.dll", "qcap.dll", "qdv.dll", "qdvd.dll", "qedit.dll", "qedwipes.dll", "qmgr.dll", "qualityupdateassistant.dll", "quartz.dll", "query.dll", "quickactionsdatamodel.dll", "quiethours.dll", "qwave.dll", "racengn.dll", "racpldlg.dll", "radardt.dll", "radarrs.dll", "radcui.dll", "randomaccessstreamdatasource.dll", "rasadhlp.dll", "rasapi32.dll", "rasauto.dll", "raschap.dll", "raschapext.dll", "rasctrs.dll", "rascustom.dll", "rasdiag.dll", "rasdlg.dll", "rasgcw.dll", "rasman.dll", "rasmans.dll", "rasmbmgr.dll", "rasmediamanager.dll", "rasmm.dll", "rasmontr.dll", "rasplap.dll", "rasppp.dll", "rastapi.dll", "rastls.dll", "rastlsext.dll", "rdbui.dll", "rdp4vs.dll", "rdpavenc.dll", "rdpbase.dll", "rdpcfgex.dll", "rdpcorets.dll", "rdpcredentialprovider.dll", "rdpendp.dll", "rdpnanotransport.dll", "rdprelaytransport.dll", "rdpsaps.dll", "rdpserverbase.dll", "rdpsharercom.dll", "rdpudd.dll", "rdpviewerax.dll", "rdsappxhelper.dll", "rdsdwmdr.dll", "rdvvmtransport.dll", "rdxservice.dll", "rdxtaskfactory.dll", "reagent.dll", "reagenttask.dll", "recovery.dll", "regapi.dll", "regctrl.dll", "regidle.dll", "regsvc.dll", "reguwpapi.dll", "reinfo.dll", "remoteaudioendpoint.dll", "remotepg.dll", "remotewipecsp.dll", "removablemediaprovisioningplugin.dll", "removedevicecontexthandler.dll", "removedeviceelevated.dll", "reportingcsp.dll", "resbparser.dll", "reseteng.dll", "resetengine.dll", "resetengonline.dll", "resourcemapper.dll", "resourcepolicyclient.dll", "resourcepolicyserver.dll", "resutils.dll", "rgb9rast.dll", "riched20.dll", "riched32.dll", "rjvmdmconfig.dll", "rmapi.dll", "rmclient.dll", "rmsroamingsecurity.dll", "rnr20.dll", "roamingsecurity.dll", "rometadata.dll", "rotmgr.dll", "rpcepmap.dll", "rpchttp.dll", "rpcns4.dll", "rpcnsh.dll", "rpcrt4.dll", "rpcrtremote.dll", "rpcss.dll", "rsaenh.dll", "rshx32.dll", "rstrtmgr.dll", "rtffilt.dll", "rtm.dll", "rtmcodecs.dll", "rtmediaframe.dll", "rtmmvrortc.dll", "rtmpal.dll", "rtmpltfm.dll", "rtpm.dll", "rtutils.dll", "rtworkq.dll", "rulebasedds.dll", "samcli.dll", "samlib.dll", "samsrv.dll", "sas.dll", "sbe.dll", "sbeio.dll", "sberes.dll", "sbresources.dll", "sbservicetrigger.dll", "scansetting.dll", "scardbi.dll", "scarddlg.dll", "scardsvr.dll", "scavengeui.dll", "scdeviceenum.dll", "scecli.dll", "scesrv.dll", "schannel.dll", "schedcli.dll", "schedsvc.dll", "scksp.dll", "scripto.dll", "scrobj.dll", "scrptadm.dll", "scrrun.dll", "sdcpl.dll", "sdds.dll", "sdengin2.dll", "sdfhost.dll", "sdhcinst.dll", "sdiageng.dll", "sdiagprv.dll", "sdiagschd.dll", "sdohlp.dll", "sdrsvc.dll", "sdshext.dll", "search.protocolhandler.mapi2.dll", "searchfolder.dll", "searchindexercore.dll", "sebbackgroundmanagerpolicy.dll", "seceditctl.bcm.x64.dll", "secfw_authenticamd.dll", "sechost.dll", "seclogon.dll", "secproc.dll", "secproc_isv.dll", "secproc_ssp.dll", "secproc_ssp_isv.dll", "secur32.dll", "securetimeaggregator.dll", "security.dll", "securitycenterbroker.dll", "securitycenterbrokerps.dll", "securityhealthagent.dll", "securityhealthcore.dll", "securityhealthproxystub.dll", "securityhealthsso.dll", "securityhealthssoudk.dll", "securityhealthudk.dll", "sedplugins.dll", "semgrps.dll", "semgrsvc.dll", "sendmail.dll", "sens.dll", "sensapi.dll", "sensorperformanceevents.dll", "sensorsapi.dll", "sensorsclassextension.dll", "sensorscpl.dll", "sensorservice.dll", "sensorsnativeapi.dll", "sensorsnativeapi.v2.dll", "sensorsutilsv2.dll", "sensrsvc.dll", "serialui.dll", "servicingcommon.dll", "servicinguapi.dll", "serwvdrv.dll", "sessenv.dll", "setbcdlocale.dll", "setnetworklocation.dll", "setnetworklocationflyout.dll", "setproxycredential.dll", "settingsenvironment.desktop.dll", "settingsextensibilityhandlers.dll", "settingshandlers_accessibility.dll", "settingshandlers_advertisingid.dll", "settingshandlers_analogshell.dll", "settingshandlers_appcontrol.dll", "settingshandlers_appexecutionalias.dll", "settingshandlers_authentication.dll", "settingshandlers_backgroundapps.dll", "settingshandlers_backup.dll", "settingshandlers_batteryusage.dll", "settingshandlers_camera.dll", "settingshandlers_capabilityaccess.dll", "settingshandlers_clipboard.dll", "settingshandlers_closedcaptioning.dll", "settingshandlers_cloudpc.dll", "settingshandlers_contentdeliverymanager.dll", "settingshandlers_cortana.dll", "settingshandlers_desktoptaskbar.dll", "settingshandlers_devices.dll", "settingshandlers_display.dll", "settingshandlers_flights.dll", "settingshandlers_fonts.dll", "settingshandlers_forcesync.dll", "settingshandlers_gaming.dll", "settingshandlers_geolocation.dll", "settingshandlers_gpu.dll", "settingshandlers_hololens_environment.dll", "settingshandlers_humanpresence.dll", "settingshandlers_ime.dll", "settingshandlers_inkingtypingprivacy.dll", "settingshandlers_inputpersonalization.dll", "settingshandlers_installedupdates.dll", "settingshandlers_keyboard.dll", "settingshandlers_language.dll", "settingshandlers_lighting.dll", "settingshandlers_managephone.dll", "settingshandlers_maps.dll", "settingshandlers_mouse.dll", "settingshandlers_notifications.dll", "settingshandlers_nt.dll", "settingshandlers_onecore_batterysaver.dll", "settingshandlers_onecore_powerandsleep.dll", "settingshandlers_onedrivebackup.dll", "settingshandlers_optionalfeatures.dll", "settingshandlers_pcdisplay.dll", "settingshandlers_pen.dll", "settingshandlers_region.dll", "settingshandlers_sharedexperiences_rome.dll", "settingshandlers_siuf.dll", "settingshandlers_speechprivacy.dll", "settingshandlers_startup.dll", "settingshandlers_storage.dll", "settingshandlers_storagesense.dll", "settingshandlers_touch.dll", "settingshandlers_troubleshoot.dll", "settingshandlers_user.dll", "settingshandlers_useraccount.dll", "settingshandlers_userexperience.dll", "settingshandlers_userintent.dll", "settingshandlers_workaccess.dll", "settingsyncdownloadhelper.dll", "setupapi.dll", "setupcl.dll", "setupcln.dll", "setupetw.dll", "sfape.dll", "sfapm.dll", "sfc.dll", "sfc_os.dll", "sgl_mnn_dll.dll", "shacct.dll", "shacctprofile.dll", "sharedpccsp.dll", "sharedrealitysvc.dll", "sharehost.dll", "sharemediacpl.dll", "shcore.dll", "shdocvw.dll", "shell32.dll", "shellcommoncommonproxystub.dll", "shellstyle.dll", "shfolder.dll", "shgina.dll", "shimeng.dll", "shimgvw.dll", "shlwapi.dll", "shpafact.dll", "shsetup.dll", "shsvcs.dll", "shunimpl.dll", "shutdownext.dll", "shutdownux.dll", "shwebsvc.dll", "signdrv.dll", "simauth.dll", "simcfg.dll", "skci.dll", "slc.dll", "slcext.dll", "slwga.dll", "smartactionplatform.dll", "smartcardbackgroundpolicy.dll", "smartcardcredentialprovider.dll", "smartcardsimulator.dll", "smartscreen.dll", "smartscreenps.dll", "smartworkflows.dll", "smbhelperclass.dll", "smbwmiv2.dll", "smiengine.dll", "smphost.dll", "smsroutersvc.dll", "sndvolsso.dll", "snmpapi.dll", "socialapis.dll", "softkbd.dll", "softpub.dll", "sortserver2003compat.dll", "sortwindows61.dll", "sortwindows62.dll", "sortwindows63.dll", "sortwindows6compat.dll", "spacecontrol.dll", "spatialinteraction.dll", "spatializerapo.dll", "spatialstore.dll", "spbcd.dll", "spectrumsyncclient.dll", "spfileq.dll", "spinf.dll", "spitdevmft64.dll", "spmpm.dll", "spnet.dll", "spoolss.dll", "spopk.dll", "spp.dll", "sppc.dll", "sppcext.dll", "sppcomapi.dll", "sppcommdlg.dll", "sppnp.dll", "sppobjs.dll", "sppwinob.dll", "sppwmi.dll", "spwinsat.dll", "spwizeng.dll", "spwizimg.dll", "spwizres.dll", "spwmp.dll", "sqlncli11.dll", "sqlserverspatial120.dll", "sqlserverspatial150.dll", "sqlsrv32.dll", "sqmapi.dll", "srchadmin.dll", "srclient.dll", "srcore.dll", "srevents.dll", "srh.dll", "srhelper.dll", "srpapi.dll", "srpuxnativesnapin.dll", "srrstr.dll", "srumapi.dll", "srumsvc.dll", "srvcli.dll", "srvsvc.dll", "srwmi.dll", "sscore.dll", "sscoreext.dll", "ssdm.dll", "ssdpapi.dll", "ssdpsrv.dll", "sspicli.dll", "sspisrv.dll", "ssshim.dll", "sstpcfg.dll", "sstpsvc.dll", "starttiledata.dll", "startupscan.dll", "staterepository.core.dll", "stclient.dll", "sti.dll", "sti_ci.dll", "stobject.dll", "storagecontexthandler.dll", "storageusage.dll", "storagewmi.dll", "storagewmi_passthru.dll", "storewuauth.dll", "storprop.dll", "storsvc.dll", "streamci.dll", "stringfeedbackengine.dll", "structuredquery.dll", "sud.dll", "sustainabilityservice.dll", "svf.dll", "svsvc.dll", "switcherdatamodel.dll", "swprv.dll", "sxproxy.dll", "sxs.dll", "sxshared.dll", "sxssrv.dll", "sxsstore.dll", "synccenter.dll", "synccontroller.dll", "synchostps.dll", "syncinfrastructure.dll", "syncinfrastructureps.dll", "syncproxy.dll", "syncreg.dll", "syncres.dll", "syncsettings.dll", "syncutil.dll", "sysclass.dll", "sysfxui.dll", "sysmain.dll", "sysntfy.dll", "syssetup.dll", "systemcpl.dll", "systemeventsbrokerclient.dll", "systemeventsbrokerserver.dll", "systemsettings.datamodel.dll", "systemsettings.deviceencryptionhandlers.dll", "systemsettings.handlers.dll", "systemsettings.settingsextensibility.dll", "systemsettings.useraccountshandlers.dll", "systemsettingsthresholdadminflowui.dll", "systemsupportinfo.dll", "t2embed.dll", "tabbtn.dll", "tabbtnex.dll", "tabsvc.dll", "tapi3.dll", "tapi32.dll", "tapilua.dll", "tapimigplugin.dll", "tapiperf.dll", "tapisrv.dll", "tapisysprep.dll", "tapiui.dll", "taskapis.dll", "taskbar.dll", "taskbarcpl.dll", "taskcomp.dll", "taskflowdataengine.dll", "taskmanagerdatalayer.dll", "taskschd.dll", "taskschdps.dll", "tbauth.dll", "tbb.dll", "tbs.dll", "tcbloader.dll", "tcpipcfg.dll", "tcpmib.dll", "tcpmon.dll", "tcpmonui.dll", "tdh.dll", "tdhres.dll", "tdlmigration.dll", "teemanagement64.dll", "telephonyinteractiveuser.dll", "telephonyinteractiveuserres.dll", "tempsignedlicenseexchangetask.dll", "termmgr.dll", "termsrv.dll", "tetheringclient.dll", "tetheringconfigsp.dll", "tetheringieprovider.dll", "tetheringmgr.dll", "tetheringservice.dll", "tetheringstation.dll", "textinputframework.dll", "textinputmethodformatter.dll", "textshaping.dll", "themecpl.dll", "themes.ssfdownload.scheduledtask.dll", "themeservice.dll", "themeui.dll", "threadpoolwinrt.dll", "threatassessment.dll", "threatexperiencemanager.dll", "threatintelligence.dll", "threatresponseengine.dll", "thumbcache.dll", "tier2punctuations.dll", "tieringengineproxy.dll", "tiledatarepository.dll", "timebrokerclient.dll", "timebrokerserver.dll", "timedatemuicallback.dll", "timesync.dll", "timesynctask.dll", "tlscsp.dll", "tokenbinding.dll", "tokenbroker.dll", "tokenbrokerui.dll", "tpmcertresources.dll", "tpmcompc.dll", "tpmcoreprovisioning.dll", "tpmengum.dll", "tpmengum138.dll", "tpmtasks.dll", "tpmvsc.dll", "tprtdll.dll", "tquery.dll", "traffic.dll", "transliterationranker.dll", "trie.dll", "trkwks.dll", "trustedsignalcredprov.dll", "tsbyuv.dll", "tsf3gip.dll", "tsgqec.dll", "tsmf.dll", "tspkg.dll", "tssessionux.dll", "tsusbgdcoinstaller.dll", "tsusbredirectiongrouppolicyextension.dll", "tsworkspace.dll", "ttdloader.dll", "ttdplm.dll", "ttdrecord.dll", "ttdrecordcpu.dll", "ttlsauth.dll", "ttlscfg.dll", "ttlsext.dll", "tvratings.dll", "twext.dll", "twinapi.appcore.dll", "twinapi.dll", "twinui.appcore.dll", "twinui.dll", "twinui.pcshell.dll", "txflog.dll", "txfw32.dll", "tzautoupdate.dll", "tzres.dll", "tzsyncres.dll", "ubpm.dll", "ucmhc.dll", "ucrtbase.dll", "ucrtbased.dll", "ucrtbase_clr0400.dll", "ucrtbase_enclave.dll", "udhisapi.dll", "udwm.dll", "ueficsp.dll", "uexfat.dll", "ufat.dll", "uiamanager.dll", "uianimation.dll", "uiautomationcore.dll", "uicom.dll", "uimanagerbrokerps.dll", "uireng.dll", "uiribbon.dll", "uiribbonres.dll", "ulib.dll", "umb.dll", "umdmxfrm.dll", "umpdc.dll", "umpnpmgr.dll", "umpo-overrides.dll", "umpo.dll", "umpodev.dll", "umpoext.dll", "umpowmi.dll", "umrdp.dll", "unattend.dll", "unenrollhook.dll", "unifiedconsent.dll", "unimdmat.dll", "uniplat.dll", "unistore.dll", "untfs.dll", "updateagent.dll", "updatecsp.dll", "updateheartbeatscan.dll", "updatepolicy.dll", "updatepolicyscenarioreliabilityaggregator.dll", "updatereboot.dll", "upnp.dll", "upnphost.dll", "upprinterinstallscsp.dll", "upshared.dll", "urefs.dll", "urefsv1.dll", "ureg.dll", "url.dll", "urlmon.dll", "usbcapi.dll", "usbceip.dll", "usbmon.dll", "usbperf.dll", "usbpmapi.dll", "usbsettingshandlers.dll", "usbtask.dll", "usbui.dll", "user32.dll", "useraccountcontrolsettings.dll", "useractivitybroker.dll", "usercpl.dll", "userdataaccessres.dll", "userdataaccountapis.dll", "userdatalanguageutil.dll", "userdataplatformhelperutil.dll", "userdataservice.dll", "userdatatimeutil.dll", "userdatatypehelperutil.dll", "userdeviceregistration.dll", "userdeviceregistration.ngc.dll", "userenv.dll", "userinitext.dll", "userlanguageprofilecallback.dll", "usermgr.dll", "usermgrcli.dll", "usermgrproxy.dll", "usoapi.dll", "usocoreps.dll", "usodocked.dll", "usosvc.dll", "usosvcimpl.dll", "usp10.dll", "ustprov.dll", "utcapi.dll", "utcutil.dll", "utildll.dll", "uudf.dll", "uvcmodel.dll", "uxinit.dll", "uxlib.dll", "uxlibres.dll", "uxtheme.dll", "vac.dll", "van.dll", "vault.dll", "vaultcds.dll", "vaultcli.dll", "vaultroaming.dll", "vaultsvc.dll", "vbsapi.dll", "vbscript.dll", "vbssysprep.dll", "vcamp110.dll", "vcamp120.dll", "vcamp140.dll", "vcamp140d.dll", "vcardparser.dll", "vccorlib110.dll", "vccorlib120.dll", "vccorlib140.dll", "vccorlib140d.dll", "vcomp100.dll", "vcomp110.dll", "vcomp120.dll", "vcomp140.dll", "vcomp140d.dll", "vcruntime140.dll", "vcruntime140d.dll", "vcruntime140_1.dll", "vcruntime140_1d.dll", "vcruntime140_1_clr0400.dll", "vcruntime140_clr0400.dll", "vcruntime140_threads.dll", "vcruntime140_threadsd.dll", "vdsbas.dll", "vdsdyn.dll", "vdsutil.dll", "vdsvd.dll", "vds_ps.dll", "verifier.dll", "version.dll", "vertdll.dll", "vfbasics.dll", "vfcompat.dll", "vfcuzz.dll", "vfluapriv.dll", "vfnet.dll", "vfntlmless.dll", "vfnws.dll", "vfpapi.dll", "vfprint.dll", "vfprintpthelper.dll", "vfrdvcompat.dll", "vfuprov.dll", "vfwwdm32.dll", "vhfum.dll", "vid.dll", "videohandlers.dll", "virtdisk.dll", "virtualmonitormanager.dll", "virtualsurroundapo.dll", "vmapplicationhealthmonitorproxy.dll", "vmbuspipe.dll", "vmbuspiper.dll", "vmbusvdev.dll", "vmchipset.dll", "vmcompute.dll", "vmcomputeeventlog.dll", "vmcrashdump.dll", "vmdatastore.dll", "vmdebug.dll", "vmdevicehost.dll", "vmdynmem.dll", "vmemulateddevices.dll", "vmemulatednic.dll", "vmemulatedstorage.dll", "vmfirmware.dll", "vmfirmwarehcl.dll", "vmfirmwarepcat.dll", "vmflexio.dll", "vmhbmgmt.dll", "vmhgs.dll", "vmiccore.dll", "vmicrdv.dll", "vmictimeprovider.dll", "vmicvdev.dll", "vmmsprox.dll", "vmpmem.dll", "vmprox.dll", "vmrdvcore.dll", "vmserial.dll", "vmsif.dll", "vmsifcore.dll", "vmsifproxystub.dll", "vmsmb.dll", "vmsynthfcvdev.dll", "vmsynthnic.dll", "vmsynthstor.dll", "vmtpm.dll", "vmuidevices.dll", "vmusrv.dll", "vmvirtio.dll", "vmvpci.dll", "vmwpctrl.dll", "vmwpevents.dll", "vocabroaminghandler.dll", "voiceactivationmanager.dll", "voiprt.dll", "vp9fs.dll", "vpcievdev.dll", "vpnike.dll", "vpnikeapi.dll", "vpnsohdesktop.dll", "vpnv2csp.dll", "vrdumed.dll", "vrfcore.dll", "vscmgrps.dll", "vsconfig.dll", "vscover170.dll", "vsd3dwarpdebug.dll", "vsgraphicscapture.dll", "vsgraphicsexperiment.dll", "vsgraphicshelper.dll", "vsgraphicsproxystub.dll", "vsperf170.dll", "vssapi.dll", "vsstrace.dll", "vss_ps.dll", "vulkan-1-999-0-0-0.dll", "vulkan-1.dll", "w32time.dll", "w32topl.dll", "waasassessment.dll", "waasmedicps.dll", "waasmedicsvc.dll", "wabsyncprovider.dll", "walletbackgroundserviceproxy.dll", "walletproxy.dll", "walletservice.dll", "wamregps.dll", "wavemsp.dll", "wbemcomn.dll", "wbiosrvc.dll", "wci.dll", "wcimage.dll", "wcmapi.dll", "wcmcsp.dll", "wcmsvc.dll", "wcnapi.dll", "wcncsvc.dll", "wcneapauthproxy.dll", "wcneappeerproxy.dll", "wcnnetsh.dll", "wcnwiz.dll", "wc_storage.dll", "wdc.dll", "wdfcoinstaller01009.dll", "wdi.dll", "wdigest.dll", "wdscore.dll", "weasel.dll", "webauthn.dll", "webcamui.dll", "webcheck.dll", "webclnt.dll", "webio.dll", "webplatstorageserver.dll", "webruntimemanager.dll", "webservices.dll", "websocket.dll", "webthreatdefsvc.dll", "webthreatdefusersvc.dll", "wecapi.dll", "wecsvc.dll", "wephostsvc.dll", "wer.dll", "werconcpl.dll", "wercplsupport.dll", "werdiagcontroller.dll", "werenc.dll", "weretw.dll", "wersvc.dll", "werui.dll", "wevtapi.dll", "wevtfwd.dll", "wevtsvc.dll", "wfapigp.dll", "wfdprov.dll", "wfdsconmgr.dll", "wfdsconmgrsvc.dll", "wfhc.dll", "wfsr.dll", "whealogr.dll", "whhelper.dll", "wiaaut.dll", "wiadefui.dll", "wiadss.dll", "wiaextensionhost64.dll", "wiarpc.dll", "wiascanprofiles.dll", "wiaservc.dll", "wiashext.dll", "wiatrace.dll", "wificloudstore.dll", "wificonfigsp.dll", "wifidatacapabilityhandler.dll", "wifidisplay.dll", "wifinetworkmanager.dll", "wimgapi.dll", "win32appinventorycsp.dll", "win32compatibilityappraisercsp.dll", "win32spl.dll", "win32u.dll", "win32_deviceguard.dll", "winbio.dll", "winbiodatamodel.dll", "winbioext.dll", "winbrand.dll", "wincorlib.dll", "wincredprovider.dll", "wincredui.dll", "windlp.dll", "windowmanagement.dll", "windowmanagementapi.dll", "windows.accountscontrol.dll", "windows.ai.machinelearning.dll", "windows.ai.machinelearning.preview.dll", "windows.applicationmodel.background.systemeventsbroker.dll", "windows.applicationmodel.background.timebroker.dll", "windows.applicationmodel.conversationalagent.dll", "windows.applicationmodel.conversationalagent.internal.proxystub.dll", "windows.applicationmodel.conversationalagent.proxystub.dll", "windows.applicationmodel.core.dll", "windows.applicationmodel.datatransfer.dll", "windows.applicationmodel.dll", "windows.applicationmodel.lockscreen.dll", "windows.applicationmodel.store.dll", "windows.applicationmodel.store.preview.dosettings.dll", "windows.applicationmodel.store.testingframework.dll", "windows.applicationmodel.wallet.dll", "windows.cloudstore.dll", "windows.cloudstore.earlydownloader.dll", "windows.cloudstore.schema.desktopshell.dll", "windows.cloudstore.schema.shell.dll", "windows.cortana.desktop.dll", "windows.cortana.onecore.dll", "windows.cortana.proxystub.dll", "windows.data.activities.dll", "windows.data.pdf.dll", "windows.devices.alljoyn.dll", "windows.devices.background.dll", "windows.devices.background.ps.dll", "windows.devices.bluetooth.dll", "windows.devices.custom.dll", "windows.devices.custom.ps.dll", "windows.devices.enumeration.dll", "windows.devices.haptics.dll", "windows.devices.humaninterfacedevice.dll", "windows.devices.lights.dll", "windows.devices.lowlevel.dll", "windows.devices.midi.dll", "windows.devices.perception.dll", "windows.devices.picker.dll", "windows.devices.pointofservice.dll", "windows.devices.portable.dll", "windows.devices.printers.dll", "windows.devices.printers.extensions.dll", "windows.devices.radios.dll", "windows.devices.scanners.dll", "windows.devices.sensors.dll", "windows.devices.serialcommunication.dll", "windows.devices.smartcards.dll", "windows.devices.smartcards.phone.dll", "windows.devices.usb.dll", "windows.devices.wifi.dll", "windows.devices.wifidirect.dll", "windows.energy.dll", "windows.fileexplorer.common.dll", "windows.gaming.input.dll", "windows.gaming.preview.dll", "windows.gaming.ui.gamebar.dll", "windows.gaming.xboxlive.storage.dll", "windows.globalization.dll", "windows.globalization.fontgroups.dll", "windows.globalization.phonenumberformatting.dll", "windows.graphics.display.brightnessoverride.dll", "windows.graphics.display.displayenhancementoverride.dll", "windows.graphics.dll", "windows.graphics.printing.3d.dll", "windows.graphics.printing.dll", "windows.graphics.printing.workflow.dll", "windows.graphics.printing.workflow.native.dll", "windows.help.runtime.dll", "windows.immersiveshell.serviceprovider.dll", "windows.internal.adaptivecards.xamlcardrenderer.dll", "windows.internal.capturepicker.desktop.dll", "windows.internal.capturepicker.dll", "windows.internal.devices.bluetooth.dll", "windows.internal.devices.sensors.dll", "windows.internal.feedback.analog.dll", "windows.internal.feedback.analog.proxystub.dll", "windows.internal.graphics.display.displaycolormanagement.dll", "windows.internal.graphics.display.displayenhancementmanagement.dll", "windows.internal.hardwareconfirmator.dll", "windows.internal.management.dll", "windows.internal.openwithhost.dll", "windows.internal.platformextension.devicepickerexperience.dll", "windows.internal.platformextension.miracastbannerexperience.dll", "windows.internal.predictionunit.dll", "windows.internal.security.attestation.deviceattestation.dll", "windows.internal.securitymitigationsbroker.dll", "windows.internal.shell.broker.dll", "windows.internal.shell.clouddesktop.transitionscreen.dll", "windows.internal.shell.xamlinputviewhost.dll", "windows.internal.shellcommon.accountscontrolexperience.dll", "windows.internal.shellcommon.appresolvermodal.dll", "windows.internal.shellcommon.broker.dll", "windows.internal.shellcommon.dll", "windows.internal.shellcommon.filepickerexperiencemem.dll", "windows.internal.shellcommon.printexperience.dll", "windows.internal.shellcommon.shareexperience.dll", "windows.internal.shellcommon.tokenbrokermodal.dll", "windows.internal.signals.dll", "windows.internal.system.userprofile.dll", "windows.internal.taskbar.dll", "windows.internal.ui.bioenrollment.proxystub.dll", "windows.internal.ui.dialogs.dll", "windows.internal.ui.logon.proxystub.dll", "windows.internal.ui.shell.windowtabmanager.dll", "windows.internal.waasmedicdocked.dll", "windows.management.enrollmentstatustracking.configprovider.dll", "windows.management.inprocobjects.dll", "windows.management.moderndeployment.configproviders.dll", "windows.management.provisioning.proxystub.dll", "windows.management.service.dll", "windows.management.update.dll", "windows.management.workplace.dll", "windows.management.workplace.workplacesettings.dll", "windows.media.audio.dll", "windows.media.backgroundmediaplayback.dll", "windows.media.devices.dll", "windows.media.dll", "windows.media.editing.dll", "windows.media.faceanalysis.dll", "windows.media.import.dll", "windows.media.mediacontrol.dll", "windows.media.mixedrealitycapture.dll", "windows.media.ocr.dll", "windows.media.playback.backgroundmediaplayer.dll", "windows.media.playback.mediaplayer.dll", "windows.media.playback.proxystub.dll", "windows.media.protection.playready.dll", "windows.media.renewal.dll", "windows.media.speech.dll", "windows.media.speech.uxres.dll", "windows.media.streaming.dll", "windows.media.streaming.ps.dll", "windows.mirage.dll", "windows.mirage.internal.dll", "windows.networking.backgroundtransfer.backgroundmanagerpolicy.dll", "windows.networking.backgroundtransfer.contentprefetchtask.dll", "windows.networking.backgroundtransfer.dll", "windows.networking.connectivity.dll", "windows.networking.dll", "windows.networking.hostname.dll", "windows.networking.networkoperators.esim.dll", "windows.networking.networkoperators.hotspotauthentication.dll", "windows.networking.proximity.dll", "windows.networking.servicediscovery.dnssd.dll", "windows.networking.sockets.pushenabledapplication.dll", "windows.networking.ux.eaprequesthandler.dll", "windows.networking.vpn.dll", "windows.networking.xboxlive.proxystub.dll", "windows.payments.dll", "windows.perception.stub.dll", "windows.security.authentication.identity.provider.dll", "windows.security.authentication.onlineid.dll", "windows.security.authentication.web.core.dll", "windows.security.credentials.ui.credentialpicker.dll", "windows.security.credentials.ui.userconsentverifier.dll", "windows.security.integrity.dll", "windows.services.targetedcontent.dll", "windows.sharedpc.accountmanager.dll", "windows.sharedpc.credentialprovider.dll", "windows.shell.bluelightreduction.dll", "windows.shell.servicehostbuilder.dll", "windows.shell.startlayoutpopulationevents.dll", "windows.staterepository.dll", "windows.staterepositorybroker.dll", "windows.staterepositoryclient.dll", "windows.staterepositorycore.dll", "windows.staterepositoryps.dll", "windows.staterepositoryupgrade.dll", "windows.storage.applicationdata.dll", "windows.storage.compression.dll", "windows.storage.dll", "windows.storage.onecore.dll", "windows.storage.search.dll", "windows.system.diagnostics.dll", "windows.system.diagnostics.telemetry.platformtelemetryclient.dll", "windows.system.diagnostics.tracereporting.platformdiagnosticactions.dll", "windows.system.launcher.dll", "windows.system.profile.hardwareid.dll", "windows.system.profile.platformdiagnosticsandusagedatasettings.dll", "windows.system.profile.retailinfo.dll", "windows.system.profile.systemid.dll", "windows.system.profile.systemmanufacturers.dll", "windows.system.remotedesktop.dll", "windows.system.systemmanagement.dll", "windows.system.userdeviceassociation.dll", "windows.system.userprofile.diagnosticssettings.dll", "windows.ui.accessibility.dll", "windows.ui.appdefaults.dll", "windows.ui.biofeedback.dll", "windows.ui.blockedshutdown.dll", "windows.ui.core.textinput.dll", "windows.ui.cred.dll", "windows.ui.creddialogcontroller.dll", "windows.ui.dll", "windows.ui.fileexplorer.dll", "windows.ui.immersive.dll", "windows.ui.input.inking.analysis.dll", "windows.ui.input.inking.dll", "windows.ui.logon.dll", "windows.ui.networkuxcontroller.dll", "windows.ui.picturepassword.dll", "windows.ui.search.dll", "windows.ui.shell.dll", "windows.ui.shell.internal.adaptivecards.dll", "windows.ui.storage.dll", "windows.ui.xaml.controls.dll", "windows.ui.xaml.dll", "windows.ui.xaml.inkcontrols.dll", "windows.ui.xaml.maps.dll", "windows.ui.xaml.phone.dll", "windows.ui.xaml.resources.19h1.dll", "windows.ui.xaml.resources.21h1.dll", "windows.ui.xaml.resources.common.dll", "windows.ui.xaml.resources.rs1.dll", "windows.ui.xaml.resources.rs2.dll", "windows.ui.xaml.resources.rs3.dll", "windows.ui.xaml.resources.rs4.dll", "windows.ui.xaml.resources.rs5.dll", "windows.ui.xaml.resources.th.dll", "windows.ui.xaml.resources.win81.dll", "windows.ui.xaml.resources.win8rtm.dll", "windows.ui.xamlhost.dll", "windows.warp.jitservice.dll", "windows.web.diagnostics.dll", "windows.web.dll", "windows.web.http.dll", "windowsaccessbridge-64.dll", "windowscodecs.dll", "windowscodecsext.dll", "windowsdefaultheatprocessor.dll", "windowsinternal.composableshell.display.dll", "windowsinternal.shell.compuiactivation.dll", "windowslivelogin.dll", "windowsmanagementservicewinrt.proxystub.dll", "windowsperformancerecordercontrol.dll", "windowsudk.shellcommon.dll", "windowsudkservices.shellcommon.dll", "winethc.dll", "winfax.dll", "winhttp.dll", "winhttpcom.dll", "winhvemulation.dll", "winhvplatform.dll", "wininet.dll", "wininetlui.dll", "wininitext.dll", "winipcfile.dll", "winipcsecproc.dll", "winipsec.dll", "winlangdb.dll", "winlogonext.dll", "winmde.dll", "winml.dll", "winmm.dll", "winmmbase.dll", "winmsipc.dll", "winmsoirmprotector.dll", "winnlsres.dll", "winnsi.dll", "winopcirmprotector.dll", "winreagent.dll", "winrnr.dll", "winrscmd.dll", "winrsmgr.dll", "winrssrv.dll", "winrttracing.dll", "winsatapi.dll", "winscard.dll", "winshfhc.dll", "winsku.dll", "winsockhc.dll", "winsqlite3.dll", "winsrv.dll", "winsrvext.dll", "winsta.dll", "winsync.dll", "winsyncmetastore.dll", "winsyncproviders.dll", "wintrust.dll", "wintypes.dll", "winuicohabitation.dll", "winusb.dll", "winusbcoinstaller2.dll", "wirednetworkcsp.dll", "wisp.dll", "witnesswmiv2provider.dll", "wkscli.dll", "wkspbrokerax.dll", "wksprtps.dll", "wkssvc.dll", "wlanapi.dll", "wlancfg.dll", "wlanconn.dll", "wlandlg.dll", "wlangpui.dll", "wlanhc.dll", "wlanhlp.dll", "wlanmediamanager.dll", "wlanmm.dll", "wlanmsm.dll", "wlanpref.dll", "wlanradiomanager.dll", "wlansec.dll", "wlansvc.dll", "wlansvcpal.dll", "wlanui.dll", "wlanutil.dll", "wldap32.dll", "wldp.dll", "wlgpclnt.dll", "wlidcli.dll", "wlidcredprov.dll", "wlidfdp.dll", "wlidnsp.dll", "wlidprov.dll", "wlidres.dll", "wlidsvc.dll", "wmalfxgfxdsp.dll", "wmcodecdspps.dll", "wmdmlog.dll", "wmdmps.dll", "wmdrmsdk.dll", "wmerror.dll", "wmi.dll", "wmiclnt.dll", "wmidcom.dll", "wmidx.dll", "wmiprop.dll", "wmitomi.dll", "wmnetmgr.dll", "wmp.dll", "wmpdui.dll", "wmpdxm.dll", "wmpeffects.dll", "wmphoto.dll", "wmpps.dll", "wmpshell.dll", "wmsgapi.dll", "wmvdspa.dll", "woftasks.dll", "wofutil.dll", "wordbreakers.dll", "workfolderscontrol.dll", "workfoldersgpext.dll", "workfoldersres.dll", "workfoldersshell.dll", "workfolderssvc.dll", "wosc.dll", "wow64.dll", "wow64base.dll", "wow64con.dll", "wow64cpu.dll", "wow64win.dll", "wpbcreds.dll", "wpc.dll", "wpcapi.dll", "wpcdesktopmonsvc.dll", "wpcproxystubs.dll", "wpcrefreshtask.dll", "wpcwebfilter.dll", "wpdbusenum.dll", "wpdmtp.dll", "wpdmtpus.dll", "wpdshext.dll", "wpdshserviceobj.dll", "wpdsp.dll", "wpd_ci.dll", "wpnapps.dll", "wpnclient.dll", "wpncore.dll", "wpninprc.dll", "wpnprv.dll", "wpnservice.dll", "wpnsruprov.dll", "wpnuserservice.dll", "wpportinglibrary.dll", "wpprecorderum.dll", "wptaskscheduler.dll", "wpx.dll", "ws2help.dll", "ws2_32.dll", "wscapi.dll", "wscinterop.dll", "wscisvif.dll", "wsclient.dll", "wscproxystub.dll", "wscsvc.dll", "wsdapi.dll", "wsdchngr.dll", "wsdproviderutil.dll", "wsdscanproxy.dll", "wsecedit.dll", "wsepno.dll", "wshbth.dll", "wshcon.dll", "wshelper.dll", "wshext.dll", "wshhyperv.dll", "wship6.dll", "wshqos.dll", "wshrm.dll", "wshunix.dll", "wslapi.dll", "wsmagent.dll", "wsmanmigrationplugin.dll", "wsmauto.dll", "wsmplpxy.dll", "wsmres.dll", "wsmsvc.dll", "wsmwmipl.dll", "wsnmp32.dll", "wsock32.dll", "wsplib.dll", "wsp_fs.dll", "wsp_health.dll", "wsp_sr.dll", "wtdccm.dll", "wtdhost.dll", "wtdsensor.dll", "wtsapi32.dll", "wuapi.dll", "wuaueng.dll", "wuceffects.dll", "wudfcoinstaller.dll", "wudfplatform.dll", "wudfsmcclassext.dll", "wudfx.dll", "wudfx02000.dll", "wudriver.dll", "wups.dll", "wups2.dll", "wusys.dll", "wvc.dll", "wwaapi.dll", "wwaext.dll", "wwanapi.dll", "wwancfg.dll", "wwanhc.dll", "wwanprfl.dll", "wwanprotdim.dll", "wwanradiomanager.dll", "wwansvc.dll", "wwapi.dll", "x3daudio1_0.dll", "x3daudio1_7.dll", "xactengine2_1.dll", "xactengine3_7.dll", "xamltilerender.dll", "xapofx1_5.dll", "xaudio2_7.dll", "xaudio2_8.dll", "xaudio2_9.dll", "xblauthmanager.dll", "xblauthmanagerproxy.dll", "xblauthtokenbrokerext.dll", "xblgamesave.dll", "xblgamesaveext.dll", "xblgamesaveproxy.dll", "xboxgipradiomanager.dll", "xboxgipsvc.dll", "xboxgipsynthetic.dll", "xboxnetapisvc.dll", "xgameruntime.dll", "xinput1_1.dll", "xinput1_4.dll", "xinput9_1_0.dll", "xinputuap.dll", "xmlfilter.dll", "xmllite.dll", "xmlprovi.dll", "xolehlp.dll", "xpsdocumenttargetprint.dll", "xpsgdiconverter.dll", "xpsprint.dll", "xpspushlayer.dll", "xpsrasterservice.dll", "xpsservices.dll", "xpstopclmconverter.dll", "xpstopwgrconverter.dll", "xwizards.dll", "xwreg.dll", "xwtpdui.dll", "xwtpw32.dll", "ze_loader.dll", "ze_tracing_layer.dll", "ze_validation_layer.dll", "zipcontainer.dll", "zipfldr.dll", "ztrace_maps.dll", "_seceditctl.bcm.x64.dll"];

        let name = name.to_lowercase();
        for sys in SYSTEM_DLL_LIST {
            if sys == name {
                return true;
            }
        }
        false
    };
}

fn can_be_dir<P: AsRef<Path>>(path: &P) -> bool {
    if let Ok(md) = std::fs::metadata(path) {
        if md.is_dir() {
            return true;
        }
    }
    return false;
}

fn is_file<P: AsRef<Path>>(path: &P) -> bool {
    if let Ok(md) = std::fs::metadata(path) {
        return md.is_file();
    }
    return false;
}

fn get_file_format(filename: &str, objdump_loc: &str) -> String {
    let output = Command::new(objdump_loc).args(["-f", filename]).output().unwrap();

    if !output.status.success() {
        let command = format!("{objdump_loc} -f {filename}");
        eprintln!("{command} failed with error code {:?}", output.status.to_string());
        eprintln!("It failed with std error output: \n{}", String::from_utf8(output.stderr).unwrap());
        exit(1);
    }

    let output = String::from_utf8(output.stdout).unwrap().replace('\r', "");


    //println!("{}",output);
    for line in output.split("\n") {
        //println!("Line {idx}: {line}");
        if let Some(loc) = line.rfind("file format ") {
            let loc = loc + "file format ".len();
            return line[loc..line.len()].to_string();
        }
    }
    eprintln!("Failed to parse file format of {filename} from objdump output, it says: \n{output}");
    exit(1);
}

fn validate_dll(dll_loc: &Path, args: &Args, custom_validator: Option<&dyn Fn(&Path) -> Result<(), String>>) -> bool {
    if !is_file(&dll_loc) {
        return false;
    }
    if let Some(validate) = &custom_validator {
        if let Err(reason) = validate(&dll_loc) {
            if args.verbose {
                println!("Skipped \"{}\" because {reason}", dll_loc.display());
            }
            return false;
        }
    }
    return true;
}

fn search_dll_deep(name: &str, args: &Args, validate: Option<&dyn Fn(&Path) -> Result<(), String>>) -> Option<String> {
    use walkdir::WalkDir;
    for dir in args.deep_search_dirs() {
        for entry in WalkDir::new(dir) {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    if args.verbose {
                        println!("Failed to search in \"{:?}\" because {}", e.path(), e);
                    }
                    continue;
                }
            };
            let mut loc = entry.path().to_path_buf();
            loc.push(name);

            if !validate_dll(&loc, args, validate) {
                continue;
            }

            return Some(loc.to_str().unwrap().to_string());
        }
    }

    return None;
}


fn search_dll_shallow(name: &str, args: &Args, validate: Option<&dyn Fn(&Path) -> Result<(), String>>) -> Option<String> {
    for path in args.shallow_search_dirs() {
        let mut loc = PathBuf::from(path);
        loc.push(name);

        if !validate_dll(&loc, args, validate) {
            continue;
        }

        return Some(loc.to_str().unwrap().to_string());
    }
    return None;
}


fn deploy_dll(target_binary: &str, target_dir: &str, objdump_file: &str, binary_format: &str, args: &Args) {
    if args.verbose {
        println!("Deploying for \"{target_binary}\" at \"{target_dir}\"");
    }
    let deps = get_dependencies(target_binary, objdump_file);
    if args.verbose {
        println!("\"{target_binary}\" requires {:?}",deps)
    }

    for dep in &deps {
        if args.verbose {
            println!("Searching ${dep} for \"{target_binary}\"")
        }


        let expected_filename = format!("{target_dir}/{dep}");
        if let Ok(_) = std::fs::metadata(&expected_filename) {
            // the dll already exist
            if args.verbose {
                println!("{expected_filename} already exists");
            }
            continue;
        }

        if args.ignore.contains(&dep) {
            // The dll is assigned to be ignored
            if args.verbose {
                println!("Skip {dep} because it is assigned to be ignored");
            }
            continue;
        }


        if is_system_dll(dep) {
            // Skip system dll
            if args.verbose {
                println!("Skip system dll {dep}");
            }
            continue;
        }

        if !args.copy_vc_redist && is_vc_redist_dll(dep) {
            // Skip vc redist dll.
            if args.verbose {
                println!("Skip VC redistributable dll {dep}");
            }
            continue;
        }


        if args.verbose {
            println!("Searching {dep} for {target_binary}");
        }
        // search for it
        let mut loc = None;

        let validator = |loc: &Path| {
            let format = get_file_format(loc.to_str().unwrap(), objdump_file);
            if format != binary_format {
                return Err(format!("DLL architecture mismatch. Expected {binary_format}, but found {format}"));
            }
            return Ok(());
        };
        let validator = Box::new(validator);

        // try shallow search first
        if let None = &loc {
            if !args.no_shallow_search {
                if let Some(location) = search_dll_shallow(dep, args, Some(&validator)) {
                    loc = Some(location);
                }
            }
        }
        if let None = &loc {
            if !args.no_deep_search {
                if let Some(location) = search_dll_deep(dep, args, Some(&validator)) {
                    loc = Some(location);
                }
            }
        }

        if let Some(location) = &loc {
            if args.verbose {
                println!("Copying \"{location}\" to \"{target_dir}\"");
            }
            std::fs::copy(location, &expected_filename).expect("Failed to copy dll");
        } else if args.allow_missing {
            println!("Failed to find dll \"{dep}\", required by \"{target_binary}\"");
            continue;
        } else {
            eprintln!("Failed to find dll \"{dep}\", required by \"{target_binary}\"");
            exit(1);
        }


        deploy_dll(&expected_filename, target_dir, objdump_file, binary_format, args);
    }
}

fn main() {
    let mut args = Args::parse();
    {
        let target = PathBuf::from(&args.binary_file);
        if !is_file(&target) {
            eprintln!("The given target \"{}\" is not a file",target.display());
            exit(5);
        }
        if target.is_relative() {
            if args.verbose {
                print!("The given binary path \"{}\" is a relative path, ", &args.binary_file);
            }
            let mut new_target = std::env::current_dir().unwrap();
            new_target.push(target);
            let new_target = new_target.to_str().unwrap().to_string();
            if args.verbose {
                println!("converted to \"{new_target}\"")
            }
            args.binary_file = new_target;
            assert!(is_file(&args.binary_file));
        }
    }

    let objdump_loc=args.objdump_file();
    if args.verbose {
        println!("Using objdump at {objdump_loc}");
    }

    let target_dir = PathBuf::from(&args.binary_file);
    let target_dir = target_dir.parent().unwrap().to_str().unwrap();
    let format = get_file_format(&args.binary_file, &args.objdump_file());
    if args.verbose {
        println!("Binary format: \"{format}\"");
    }
    deploy_dll(&args.binary_file, target_dir, &objdump_loc, &format, &args);
}
