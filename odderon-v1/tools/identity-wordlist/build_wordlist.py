#!/usr/bin/env python3
"""Generate crates/odd-identity/src/wordlist.rs from a curated corpus.

The Flight License encodes 132 bits as twelve 11-bit indices into a 2048-word
table. Two properties make that table safe to read aloud and to autocomplete:

  * every word is 4-10 lowercase ASCII letters;
  * the first four letters are unique across the whole table, so a four-letter
    prefix always identifies exactly one word (the BIP-39 property).

The corpus below is grouped by aerospace/physics domain. Candidates that
collide on their four-letter prefix are dropped in corpus order, so earlier
groups win; put the most recognisable terms first. Re-run after editing:

    python3 tools/identity-wordlist/build_wordlist.py
"""
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
TARGET = 2048

# Order matters: on a four-letter-prefix collision the earlier word wins.
CORPUS = """
orbital orbit perigee apogee perilune apolune periapsis apoapsis perilune
apsis apsides anomaly eccentricity inclination ascending descending node
nodes argument right ascension equinox solstice ecliptic equator latitude
longitude altitude azimuth elevation zenith nadir horizon ephemeris epoch
epochal julian sidereal synodic tropical draconitic anomalistic keplerian
newtonian lagrange lagrangian hamiltonian jacobi gauss lambert hodograph
osculating perturbation perturb secular resonant resonance libration
precession nutation obliquity regression drift stationkeeping phasing
rendezvous docking berthing capture escape insertion transfer hohmann
bielliptic plane spiral loiter coast ballistic trajectory trajectories
flyby swingby gravity assist aerobrake aerocapture aerogravity skip
entry descent landing touchdown splashdown abort ascent launch liftoff
booster stage staging separation fairing payload shuttle tug spacekick
pericynthion apocynthion perilune selenocentric geocentric heliocentric
barycentric barycenter areocentric jovian kronian hermean cytherian
cis-lunar cislunar translunar transearth interplanetary interstellar
sphere hill patched conic conics ellipse ellipsoid parabolic hyperbolic
circular semimajor semiminor flattening oblateness bulge harmonic zonal
tesseral sectorial geoid geopotential gravity gravitation gravitational
tidal tides Roche magnetosphere magnetopause bowshock shockwave
spacecraft satellite probe rover lander orbiter mothership bus chassis
structure truss panel array solar wing boom mast antenna dish feed
transponder transmitter receiver downlink uplink telemetry telecommand
tracking ranging doppler delay bandwidth gain beam sidelobe polarization
link budget noise figure carrier modulation encoding decoding codec
packet frame bitstream datarate handshake protocol latency jitter
attitude gyro gyroscope reaction momentum wheel torque torquer magnetorquer
startracker sunsensor horizonSensor imu inertial accelerometer
quaternion euler rotation matrix gimbal slew pointing boresight nadir
nutation damping detumble spin spinstabilized dualspin threeaxis
thruster thrusters reactioncontrol rcs coldgas monopropellant bipropellant
hydrazine peroxide nitrogen tetroxide monomethyl unsymmetrical dimethyl
oxidizer oxidiser fuel propellant tank pressurant helium ullage feedline
valve regulator injector nozzle throat expansion bell aerospike plug
chamber combustion catalyst ignition igniter spark torch hypergolic
cryogenic cryostat boiloff chilldown subcooling densified autogenous
turbopump pump inducer impeller turbine gasgenerator preburner staged
expander pressurefed regeneratively film cooled ablative radiative
specific impulse thrust delta mass ratio drymass wetmass ullage
throttle throttleable gimballing vectored vernier ullage settling
aerodynamics aerodynamic lift drag coefficient reynolds mach supersonic
hypersonic transonic subsonic sonic shock expansion compression ramp
boundary layer laminar turbulent transition separation reattachment
vortex vorticity circulation downwash upwash wash wake slipstream
airfoil aerofoil chord camber thickness leading trailing edge flap
aileron elevator rudder stabilizer fin canard winglet spar rib skin
stringer longeron bulkhead fuselage empennage nacelle pylon intake
inlet diffuser compressor combustor turbine afterburner nozzle
static dynamic pressure pitot venturi bernoulli stagnation enthalpy
entropy entropic isentropic adiabatic isothermal isobaric isochoric
polytropic carnot rankine brayton stirling otto diesel cycle efficiency
heat thermal temperature kelvin celsius fahrenheit conduction convection
radiation emissivity absorptivity reflectivity albedo blackbody greybody
stefan boltzmann planck wien kirchhoff fourier nusselt prandtl grashof
rayleigh peclet biot film coefficient flux fluxes capacity latent
sensible specific heatpipe radiator multilayer insulation blanket
heater thermostat thermistor thermocouple louvre coating paint mirror
sublimation evaporation condensation melting freezing boiling critical
triple phase diagram mollier psychrometric humidity dewpoint saturation
vapor vapour liquid solid plasma gas ideal real compressible viscosity
viscous conductivity diffusivity diffusion knudsen mean freepath
astronomy astrophysics celestial cosmology cosmological redshift blueshift
parallax parsec lightyear astronomical unit magnitude apparent absolute
luminosity luminous flux brightness surface photometry spectrum spectral
spectroscopy spectrometer photometer polarimeter interferometer
telescope reflector refractor mirror lens objective eyepiece mount
seeing adaptive optics coronagraph occultation transit eclipse lunar
solar umbra penumbra antumbra syzygy conjunction opposition elongation
quadrature gibbous crescent newmoon fullmoon phase albedo terminator
photosphere chromosphere corona coronal prominence flare sunspot plage
facula granule supergranule helioseismology neutrino fusion fission
nucleosynthesis proton protoncycle carbon nitrogen oxygen cycle
degenerate whitedwarf neutronstar pulsar magnetar blackhole event
singularity accretion disk jet quasar blazar Seyfert starburst galaxy
nebula nebular cluster globular open association supernova nova
hypernova kilonova remnant planetary protoplanetary debris kuiper
oort asteroid comet meteor meteorite bolide chondrite achondrite
impact crater regolith ejecta rayed basin mare highland rille scarp
dorsum mons tholus patera fossa catena cavus chaos labyrinthus
planitia planum regio rupes terra undae vallis vastitas
mercury venus earth mars jupiter saturn uranus neptune pluto ceres
vesta pallas juno eros itokawa ryugu bennu didymos dimorphos europa
ganymede callisto io titan enceladus mimas tethys dione rhea iapetus
phoebe hyperion janus epimetheus pandora prometheus atlas pan telesto
calypso helene polydeuces methone anthe pallene ymir phobos deimos
triton nereid proteus larissa galatea despina thalassa naiad halimede
psamathe sao laomedeia neso charon nix hydra kerberos styx miranda
ariel umbriel titania oberon cordelia ophelia bianca cressida desdemona
juliet portia rosalind belinda puck perdita mab cupid
sirius canopus arcturus vega capella rigel procyon betelgeuse achernar
hadar altair acrux aldebaran antares spica pollux fomalhaut deneb
mimosa regulus adhara castor shaula bellatrix elnath miaplacidus
alnilam alnair alnitak alioth dubhe mirfak wezen sargas kaus avior
alkaid menkalinan atria alhena peacock mirzam alphard hamal algieba
diphda mizar nunki menkent mirach alpheratz rasalhague kochab
sadalmelik algol tiaki muhlifain aspidiske suhail alphecca mintaka
sadr eltanin schedar naos almach caph izar dschubba larawag merak
ankaa gacrux taygeta maia electra merope alcyone pleione atlas
zubenelgenubi zubeneschamali unukalhai sabik rasalgethi kornephoros
marfik saran arneb phact wazn furud mekbuda errai alfirk kurhah
alderamin garnet enif homam matar sadachbia skat albali ancha
albireo sulafat sheliak tarazed alshain denebola zosma chertan
alula tania australis megrez phegda benetnasch alkalurops alastor
orion taurus gemini cancer leo virgo libra scorpius sagittarius
capricornus aquarius pisces aries andromeda perseus auriga bootes
canesvenatici coma coronaborealis hercules lyra cygnus aquila
sagitta vulpecula delphinus equuleus pegasus lacerta cassiopeia
cepheus draco ursa ursamajor ursaminor camelopardalis lynx
cassiopeia cepheus lacerta triangulum perseus andromeda pegasus
cetus eridanus fornax sculptor phoenix grus tucana indus pavo
apus chamaeleon volans pictor dorado reticulum horologium caelum
columba lepus canismajor canisminor monoceros puppis carina vela
pyxis antlia sextans crater corvus hydra centaurus lupus crux
musca circinus norma triangulumaustrale ara telescopium microscopium
piscisaustrinus octans mensa hyrdus serpens ophiuchus scutum
libra vulpecula lynx leo minor
hydrogen helium lithium beryllium boron carbon nitrogen oxygen
fluorine neon sodium magnesium aluminium silicon phosphorus sulfur
chlorine argon potassium calcium scandium titanium vanadium chromium
manganese iron cobalt nickel copper zinc gallium germanium arsenic
selenium bromine krypton rubidium strontium yttrium zirconium niobium
molybdenum technetium ruthenium rhodium palladium silver cadmium
indium tin antimony tellurium iodine xenon cesium barium lanthanum
cerium praseodymium neodymium promethium samarium europium gadolinium
terbium dysprosium holmium erbium thulium ytterbium lutetium hafnium
tantalum tungsten rhenium osmium iridium platinum gold thallium
lead bismuth polonium astatine radon francium radium actinium
thorium protactinium uranium neptunium plutonium americium curium
berkelium californium einsteinium fermium mendelevium nobelium
lawrencium rutherfordium dubnium seaborgium bohrium hassium
meitnerium darmstadtium roentgenium copernicium nihonium flerovium
moscovium livermorium tennessine oganesson
newton euler lagrange laplace gauss kepler copernicus galileo
hubble herschel halley cassini huygens brahe ptolemy aristotle
archimedes descartes fermat pascal bernoulli poisson navier stokes
maxwell faraday ampere coulomb ohm volta tesla henry hertz lorentz
einstein bohr planck heisenberg schrodinger dirac pauli fermi
oppenheimer meitner chandrasekhar eddington hoyle gamow lemaitre
friedmann robertson walker kerr schwarzschild penrose hawking
thorne wheeler dicke sandage baade zwicky oort kuiper bode
tisserand delaunay poincare lyapunov riccati bessel legendre
chebyshev hermite jacobi laguerre taylor maclaurin fourier laplacian
runge kutta dormand prince adams bashforth moulton verlet symplectic
bulirsch stoer fehlberg cash karp radau lobatto gausslabatto
goddard tsien vonkarman oberth korolev glushko chelomei yangel
armstrong aldrin collins glenn shepard grissom schirra carpenter
slayton cooper conrad gordon bean swigert haise lovell anders
borman schweickart scott irwin worden mattingly cernan schmitt
young cripplen weitz lousma kerwin brand vance peterson musgrave
truly bobko overmyer fisher gardner allen ivins thornton parise
nelsonhawley cenker spring sherwood springer bolden hawley
ride sullivan lucid seddon fabian boekelmann norman thagard
blaha wolf lowrie voss helmsey parazynski tani bartoe lind
apollo gemini soyuz salyut skylab mir shenzhou tiangong longmarch
falcon atlas delta titan ariane vega proton angara soyuz2 antares
electron neutron newglenn vulcan terran alpha launcherOne pslv gslv
h2a epsilon kuaizhou ceresOne zhuque hyperbola tianlong shian
saturn sls orion starship dragon crewdragon cargo cygnus dreamchaser
starliner CST boreas progress atv htv konotori tianzhou
voyager pioneer mariner viking pathfinder spirit opportunity
curiosity perseverance insight phoenix lander surveyor lunar
ranger clementine prospector grace grail LRO capstone artemis
chandrayaan chang'e hayabusa osirisrex newhorizons dawn rosetta
stardust deepimpact juno cassini galileo magellan messenger
bepicolombo solarorbiter parker ulysses soho stereo sdo iris
kepler tess gaia hipparcos planck wmap cobe hubble jwst chandra
spitzer wise ska alma vla vla arecibo fast lofar meerkat
merlin raptor rutherford vulcain vinci kestrel draco ruthe be-four
rlten j-two f-one nk33 rd180 rd191 aerospike rs25 ssme lmde
ion hall xenon krypton argon arcjet resistojet magnetoplasma
vasimr nuclear thermal electric solar sail drag electrodynamic
momentum tether skyhook rotovator elevator beanstalk climber
acceleration velocity speed jerk momentum impulse energy work power
force torque stress strain shear bending torsion fatigue creep
elastic plastic yield ultimate margin factor safety load loads
structural dynamics modal frequency damping resonance flutter
divergence buzz pogo slosh waterhammer surge transient steady
quasi static equilibrium stable unstable neutral critical
linear nonlinear chaotic stochastic deterministic random noise
signal filter kalman gaussian bayesian estimator covariance
variance sigma deviation residual bias error accuracy precision
resolution quantization sampling aliasing nyquist bandwidth
telemetry frame packet checksum parity crc hash digest signature
certificate authority verify attest nonce entropy seed keypair
public private secret cipher AES chacha poly sha256 sha512 ed25519
curve secp rsa ecdsa schnorr merkle bloom lattice quantum
classical relativistic lorentz boost frame inertial rotating
coriolis centrifugal centripetal eulerian lagrangian
alpha beta gamma delta epsilon zeta eta theta iota kappa lambda
mu nu xi omicron pi rho sigma tau upsilon phi chi psi omega
meter second kilogram mole candela ampere kelvin radian steradian
hertz newton pascal joule watt coulomb volt farad ohm siemens
weber tesla henry lumen lux becquerel gray sievert katal
parsec angstrom barn electronvolt Dalton gauss maxwell oersted
gauss gal poise stokes darcy rayleigh langmuir debye
 Mach reynolds strouhal weber froude euler weissenberg deborah
nusselt prandtl schmidt lewis peclet grashof rayleigh biot
stanton colburn eckert knudsen weber capillary bond
"""


CORPUS_EXTRA = """
imbrium serenitatis crisium tranquillitatis nubium cognitum insularum
fecunditatis nectaris humorum vaporum frigoris orientale smythii marginis
undarum spumans anguis australe moscoviense ingenii aestuum epidemiarum
sinus iridum roris medii aetatis amoris somni asperitatis fidei
tycho copernicus aristarchus plato clavius grimaldi langrenus petavius
furnerius stevinus snellius rheita metius fabricius janssen maurolycus
stofler pitatus purbach arzachel alphonsus ptolemaeus hipparchus albategnius
theophilus cyrillus catharina fracastorius piccolomini rosenberger
vlacq nezval schickard schiller_longomontanus maginus longomontanus
hell gauricus deslandres wargentin mersenius billy flamsteed letronne
gassendi mersenne pythagoras philolaus anaximander anaximenes carpenter
herodotus aristoteles eudoxus alexander aristillus autolycus archimedes
timocharis euler pytheas helicon leverrier carlini diophantus delisle
angstroem bruce krieger wollastonus herigonius euclides riphaeus
agatharchides loewy darwin byrgius cavalerius hevelius seleucus eddington
krafft cardanus galilaei reiner marius aristarchus herodotus
atmosphere troposphere stratosphere mesosphere thermosphere exosphere
ionosphere ozonosphere tropopause stratopause mesopause thermopause
ionization recombination photoionization aurora airglow zodiacal
geocorona plasmasphere ringcurrent radiationbelt vanallen
convectioncell hadley ferrel polartrade monsoon chinook foehn
cyclone anticyclone hurricane typhoon tornado waterspout squall
gust microburst downburst windshear turbulence clearair mountainwave
lenticular cumulus cumulonimbus stratus cirrus nimbus altostratus
altocumulus cirrostratus cirrocumulus stratocumulus mammatus
contrail fogfront haze mist smog aerosol particulate
humidity hygrometer barometer anemometer radiosonde rawinsonde
dropsonde ceilometer nephelometer disdrometer pyranometer pyrgeometer
sounding skewtee logp hodograph cape lifted index kindex
geodesy geodetic datum ellipsoid WGS84 nadir EGM2008 geoidal
undulation deflection vertical astronomic azimuthal projection
mercator transverse utm ups stereographic gnomonic azimuthal orthographic
lambertconformal albers mollweide sinusoidal equirectangular
triangulation trilateration leveling benchmark geoidheight
gnss gps glonass galileoM beidou qzss irnss sbas waas egnos
pseudorange carrierphase ambiguity cycleSlip multipath ephemeriserror
ionodelay tropodelay dilution pdop hdop vdop gdop raim
rtk ppp insar lidar radar sonar altimeter scatterometer
materials aluminium titanium inconel monel hastelloy waspaloy rene
maraging stainless maragingsteel berylliumcopper niobiumalloy
carboncarbon silicacarbide alumina zirconia mullite ceramic
composite graphite epoxy phenolic kevlar nomex nextel quartz
fiberglass prepreg autoclave layup plyweave unidirectional
annealing quenching tempering precipitation solutionizing
welding brazing soldering diffusionbond frictionstir additive
sintering hotisostatic forging extrusion rolling machining
milling turning grinding lapping polishing electropolish
anodizing passivation alodine chromate plating sputtering
vapordeposition epitaxy implantation doping anneal
nondestructive ultrasonic radiography eddycurrent dyePenetrant
thermography shearography acoustic emission tapTest
avionics flightcomputer firmware bootloader watchdog redundancy
triplemodular voting crossstrap failoperational failsafe graceful
degradation derating burnin screening qualification acceptance
vibration pyroshock acoustic thermalvacuum bakeout outgassing
contamination cleanliness particulate molecular film
EMC EMI susceptibility conducted radiated immunity shielding
grounding bonding lightning static discharge triboelectric
battery cell lithiumion nicklehydrogen silverzinc primary secondary
stateofcharge depthofdischarge cyclelife calendarlife degradation
busbar shunt regulator peakpowertracking arrayoffpoint
fuelcell electrolyzer regenerative solararray concentrator
panelstring bypassdiode blockingdiode stringvoltage
powerconditioning converter inverter rectifier transformer
busvoltage undervoltage overcurrent crowbar fuse limiter
optics aberration spherical coma astigmatism fieldcurvature distortion
chromatic achromat apochromat aspheric parabolic cassegrain
gregorian newtonian ritchey chretien coude nasmyth primefocus
schmidt maksutov meniscus corrector fieldflattener barlow
focal fnumber aperture stop pupil entrance exit vignetting
diffraction airy rayleighcriterion strehl wavefront zernike
shackhartmann wavefrontsensor deformablemirror actuator
coating antireflection highreflect dielectric metallic protected
grating prism echelle order blaze resolution resolvingpower
detector ccd cmos bolometer photodiode avalanche photomultiplier
microchannel intensifier readnoise darkcurrent quantum efficiency
pixel binning flatfield biasframe calibration
plasma debye sheath presheath doublelayer ambipolar langmuirprobe
magnetohydrodynamic alfven whistler ionacoustic driftwave
lowerhybrid upperhybrid cyclotronfrequency gyrofrequency
reconnection tearing kink sausage interchange ballooning
tokamak stellarator spherical torus reversedfield pinch zpinch
spheromak fieldreversed mirrormachine cusp polywell fusor
iter jet wendelstein tore supra east kstar difiiiD alcatormod
nif ignition Lawson breakeven gainQ tritium deuterium breeding
blanket divertor firstwall limiter scrapeoff pedestal elmy
disruption mitigation pellet injection neutralbeam ioncyclotron
electroncyclotron lowerhybridcurrentdrive bootstrapcurrent
particle proton neutron electron positron muon pion kaon
neutrino photon gluon boson fermion hadron lepton baryon meson
quark antiquark color flavor isospin strangeness charm beauty
higgs weak strong electromagnetic interaction coupling
renormalization feynman vertex propagator loop amplitude
crosssection luminosity barn collider synchrotron cyclotron
betatron linac storage ring beamline bunch emittance
superconducting niobiumtin nitinol cryocooler pulseTube
gmcooling sorption adiabaticdemagnetization dilution
squid magnetometer hallprobe fluxgate searchcoil
quantum superposition entanglement decoherence coherence
tunneling zeroPoint vacuum casimir lambshift zeeman stark
raman compton thomson bremsstrahlung synchrotronradiation
cherenkov transitionradiation blackbody photoelectric
pairproduction annihilation bethe bloch stoppingpower
condensed lattice phonon magnon plasmon exciton polaron
bandgap fermilevel workfunction conductivity resistivity
semiconductor insulator superconductor meissner josephson
cooperpair penetrationlength coherencelength typeII
hallEffect spinHall topological weyl diracsemimetal
graphene nanotube fullerene quantumdot nanowire
crystallography bravais millerindex bragg laue debyescherrer
dislocation vacancy interstitial grainboundary twinning
martensite austenite ferrite pearlite bainite cementite
hooke young modulus poissonratio bulk shearModulus
thermalExpansion specificVolume compressibility
numerical convergence consistency stability stiffness conditioning
truncation roundoff cancellation catastrophic machineepsilon
quadrature simpson trapezoid romberg adaptive nested
clenshaw curtis gausslegendre gausslobatto kronrod
interpolation splines hermite barycentric lagrange bezier bspline
nurbs regression leastsquares normalEquation qrdecomposition
svd cholesky luFactor pivot gaussseidel jacobi sor conjugate
gradient preconditioner multigrid krylov arnoldi lanczos gmres
bicgstab minres newtonraphson secant brent bisection regula
fixedpoint contraction banach lipschitz continuity
sparse dense banded block tridiagonal pentadiagonal toeplitz
finitevolume finitedifference finiteelement spectral discontinuous
galerkin petrov collocation mesh grid refinement amr
timestep courant cfl explicit implicit semiimplicit crank
nicholson backwardsdifferentiation rosenbrock
validation verification uncertainty quantification sensitivity
montecarlo latinhypercube sobol polynomialchaos surrogate
kriging radialbasis gaussianprocess neural network
design experiment factorial taguchi response surface optimization
gradientdescent convex simplex neldermead genetic annealing
particleswarm differentialEvolution sequential quadratic
interiorpoint barrier lagrangemultiplier karushkuhn
pareto multiobjective constraint feasible infeasible
reliability weibull exponential bathtub hazard meantime
failure mode effects criticality faulttree eventtree
bowtie barrier defense redundancy diversity segregation
independence commoncause betaFactor markov availability
maintainability testability inspectability repairability
logistics spares inventory provisioning turnaround
launchpad gantry tower umbilical strongback erector transporter
flameduct deflector trench deluge water sound suppression
holdown clamp release liftoff commit countdown terminal
scrub recycle window instantaneous extended daily
range safety flighttermination destruct ordnance
trackingstation downrange impactpoint footprint corridor
notam airspace clearance maritime hazard area
mission control flightdirector capcom fido guidance officer
gnc booster retro eecom surgeon inco telmu fao
telemetry flight dynamics trajectory officer propulsion
consumables margin reserve contingency reserve budget
timeline checklist procedure callout nominal offnominal
anomaly investigation rootcause corrective action
review preliminary critical design readiness flight
qualification certification airworthiness typecertificate
supplemental exemption waiver deviation nonconformance
configuration baseline change control board
requirements specification interface control document
verification matrix traceability acceptance criteria
"""


CORPUS_EXTRA2 = """
achird acubens adhafera adhil ain aladfar alamas alathfar albaldah
albireo alchiba alcor alcyone aldebaran alderamin aldhibah aldulfin
alfirk algedi algenib algenubi algieba algiedi algorab alhajoth
alhena alhor aliquoth alkaffaljidhma alkalurops alkes alkurahah
almac almach alminliar alnair alnasl alnasr alnilam alnitak alniyat
alphard alphecca alpheratz alrai alrakis alrami alrischa alsafi
alsciaukat alshain alshat alsuhail altair altais altarf alterf
aludra alula alwaid alya alzirr ancha angetenar ankaa anser
antares arcturus arich arided arkab arneb arrakis ascella asellus
askella aspidiske asterion asterope atik atlas atria auva avior
azelfafage azha azmidi azimech baten kaitos becrux beid bellatrix
benetnasch betelgeuse botein brachium canopus capella caph celbalrai
celaeno chara cheleb chertan coxa cursa dabih decrux deneb denebola
dheneb diadem diphda dschubba dubhe duhr dziban edasich elgomeisa
elnath eltanim enif errai fomahaut furud gacrux giausar gienah
gomeisa gorgonea graffias grumium hadar hadir haedus haldus hamal
hassaleh heka heze hoedus homam hyadum hydrobius izar jabbah
jabbah jih kajam keid kitalpha kochab kornephoros kraz ksora
kuma kurhah lesath maasym mahasim maia marfark marfik markab
matar mebsuta megrez meissa mekbuda menchib menkab menkalinan
menkar menkent menkib merak merga meriah merope mesarthim miaplacidus
mimosa minchir minelauva mintaka mira mirach miram mirfak mirzam
misam mizar mothallah muliphein muphrid murzim muscida nair nashira
navi nekkar nembus neshmet nihal nunki nusakan okda peacock phact
phegda pherkad pleione pollux porrima praecipua procyon propus
proxi pulcherrimana rana rasalas raset algethi rastaban ruchba
ruchbah rukbat sabik sadachbia sadalbari sadalmelik sadalsuud sadatoni
sadr saiph salm sargas sarin sceptrum scheat schedar scheddi
segin seginus sham shaula sheliak sheratan sinistra situla skat
spica sterope sualocin subra suhail sulafat syrma tabit talitha
tania tarazed taygeta tegmine tegmen terebellum theemin thuban
tien kwan torcularis tureis tyl unuk unurgunite vega vindemiatrix
wasat wazn wezen yed prior yed posterior yildun zaniah zaurak
zavijava zhang zibal zosma zubenelgenubi zubeneschamali zubenelhakrabi
proxima rigil rigilKent toliman barnard lalande lacaille groomb
ross wolf gliese kapteyn luyten teegarden vanMaanen struve
bessel piazzi mayer bode messier dreyer bayer flamsteed hevelius
cruikshank bond draper pickering cannon fleming leavitt payne
rubin darkmatter Vera zwicky baade sandage burbidge hoyle
penzias wilson dicke hulse taylor perlmutter riess schmidt
smoot mather bennett hinshaw komatsu spergel dunkley nolta
akebono ariel asterix ausat cosmos dscovr eutelsat globalsat
gomx gonets grifex hamsat hinode hispasat hotbird inmarsat
intelsat iridiumM jason kartosat kompsat landsat meteorm meteosat
nILESAT noaa oersted oresat orsted oscarM planet pleiades
quikscat radarsat rapidEye ressources risat saral sentinelM
skysat spotM stpsat swarmM tdrs terra terraSAR thaicom
thuraya tom s topexM trmm uars worldview xmmM yaogan
baikonur plesetsk vostostochny kapustin yar kourou wenchang
xichang taiyuan jiuquan tanegashima uchinoura alcantara palmachim
sriharikota vandenberg canaveral kennedy wallop midatlantic
kodiak sandpoint whiteSands edwards mojave bocaChica starbase
cornranch mcGregor hawthorne McGregor titusville
goldstone canberra madrid malargue newNorcia usuda weilheim dongara
svalbard kiruna esac salmijarvi pokerFairbanks inuvik hartebeesthoek
kouru maspalomas dongra kikuchi weihai jiamusi kashi
greenwich jodrell parkes effelsberg greenbank socorro kittpeak
palomar wilsonM mcdonald lick yerkes lowellM flagstaff anderson
maunakea keckM subaru geminiM granTeCanarias saltM lbtM magellan
paranal cerroTololo lasCampanas laSilla armazones pachon
sutherland springfield sidingSpring skymapper wiseM neat
panStarrs catalina atlasM lincoln linearM zwickyTransient
aerodynamics airspeed groundspeed headwind tailwind crosswind
sideslip slip skid yaw pitch roll bank dihedral anhedral
sweep taper aspectRatio wingLoading stall spin spiralDive
phugoid shortPeriod dutchRoll adverseYaw proverseYaw
trim stabilator allMoving canardControl elevon flaperon
spoiler speedbrake airbrake drogue parachute ballute parafoil
glide slope approach flare roundout touchdown rollout
takeoff rotation climb cruise descent holding pattern
ils vor dme ndb tacan rnav gnssApproach circling
decisionHeight minimums goAround missedApproach balked
weightAndBalance centerOfGravity envelope payloadRange
endurance ceiling serviceCeiling absoluteCeiling rateOfClimb
bestGlide cornerVelocity maneuveringSpeed neverExceed
flutterSpeed diveSpeed buffetBoundary coffinCorner
loadFactor gustLoad limitLoad ultimateLoad fatigueLife
damageTolerance safeLife failSafe crackGrowth inspection
mineralogy olivine pyroxene plagioclase feldspar anorthosite
basalt gabbro norite troctolite dunite peridotite pyroxenite
anorthositic komatiite breccia impactMelt suevite tektite
shatterCone shockedQuartz coesite stishovite maskelynite
chondrule calciumAluminium refractory volatile siderophile
lithophile chalcophile atmophile incompatible rareEarth
isotope radiogenic cosmogenic spallogenic nucleogenic
rubidiumStrontium samariumNeodymium uraniumLead potassiumArgon
argonArgon rheniumOsmium lutetiumHafnium carbonDating
halfLife decayConstant daughter parent closureTemperature
thermochronology geothermobarometry petrology petrogenesis
magmatism volcanism cryovolcanism tectonics diapirism
subduction rifting spreading orogeny epeirogeny isostasy
flexure loading unloading rebound viscous relaxation
creepFlow dislocationGlide diffusionCreep grainBoundarySliding
rheology viscoelastic maxwellBody kelvinVoigt burgers
andrade byerlee frictional cohesion mohrCoulomb
porosity permeability tortuosity saturation capillarity
darcy flow seepage infiltration percolation outgassing
vent plume geyser cryogeyser fumarole solfatara
"""


CORPUS_EXTRA3 = """
zenithal azimuthal hourangle siderealtime localtime universal coordinated
ephemerissecond barycentricdynamical terrestrialdynamical atomic
leapsecond utcutai tai utone uttwo gmst gast era eop
polarMotion chandler wobble lengthofday nutationseries precessionmodel
iau2000 iau2006 sofа iers bulletinA bulletinB finals
framebias icrf icrs j2000 meanOfDate trueOfDate temi teme
gcrf itrf itrs ecef eci ned enu lvlh ric rsw uvw
transformation rotationmatrix directioncosine quaternionkinematics
modifiedRodrigues gibbsVector stereographicprojection attitudeerror
multiplicativeExtended additiveError kinematicPropagation
gyroBias randomWalk angleRandomWalk velocityRandomWalk allanVariance
flicker white pink brown noiseColor powerSpectral density
autocorrelation crosscorrelation coherenceFunction transferFunction
impulseResponse stepResponse settlingTime overshoot riseTime
bandwidthPhase gainMargin phaseMargin bode nyquistplot rootLocus
pid proportional integral derivative leadLag compensator
feedforward feedback openLoop closedLoop cascaded nested
observer estimator kalmanFilter extendedKalman unscented particleFilter
complementary mahony madgwick gradientDescent attitudeDetermination
quest triad foam sailRigids oliae esoq2 request
singularValue decomposition eigenvalue eigenvector spectralRadius
conditionNumber illConditioned regularization tikhonov ridge
dampingFactor naturalFrequency logarithmicDecrement qualityFactor
resonantPeak antiresonance notch lowpass highpass bandpass bandstop
butterworth chebyshev elliptic besselLinkwitz groupDelay
samplingTheorem aliasFrequency antialias decimation interpolationFilter
zeroOrderHold firstOrderHold tustin bilinear transform warping
discretization zoh matched poleZero mapping
quantizationNoise dither oversampling sigmaDelta adc dac
resolutionBits leastSignificantBit mostSignificantBit endianness
byteOrder bigEndian littleEndian serialization deserialization
marshalling unmarshalling schemaVersion backwardCompatible forward
canonicalization deterministicEncoding reproducibleBuild bitIdentical
contentAddressed merkleRoot digestAlgorithm collisionResistance
preimageResistance secondPreimage avalancheEffect diffusionConfusion
keyDerivation hkdf pbkdf2 scrypt argon2 salt pepper stretching
domainSeparation contextString versionTag protocolIdentifier
authenticatedEncryption aead gcm ccm ocb siv poly1305
keyAgreement diffieHellman ephemeral forwardSecrecy ratchet
certificateChain rootOfTrust anchor attestationQuote endorsement
secureBoot measuredBoot trustedPlatform tpm enclave
sideChannel timingAttack cacheAttack powerAnalysis faultInjection
constantTime branchless masking blinding scalarMultiplication
montgomeryLadder edwardsCurve twistedEdwards birational map
cofactor clearing torsion subgroup primeOrder smallSubgroup
malleability canonicalScalar strictVerification cofactorless
batchVerification randomizer precomputation windowed naf wnaf
comb fixedBase variableBase doubleScalar multiscalar
projective extended completed jacobian affine coordinates
doubling addition mixedAddition differential ladder
fieldInversion batchInversion montgomeryTrick squaring
reduction barrett montgomeryReduction carryless schoolbook
karatsuba toomCook numberTheoretic fastFourier
"""


CORPUS_EXTRA4 = """
kerolox methalox hydrolox aeroshell backshell heatshield windward leeward
retrofire posigrade retrograde prograde sunward ramward wakeward quench
venting purging inerting topping topoff replenish detanking defueling
safing pyrovalve frangible squib jettison aerostat airship balloon
dirigible zeppelin hypoxia ebullism barotrauma spacesuit helmet visor
glove airlock hatch spacewalk scrubber sabatier condensate potable
greywater weightless freefall vestibular otolith saccade dosimeter
circadian melatonin zeitgeber quarantine isolation crewed uncrewed
humanrated manrated skipentry bankangle crossrange aerothermo catalytic
ablation pyrolysis char recession spallation erosion oxidation
heatsink peltier seebeck thermionic kilopower reactor moderator
controlrod poison keff enrichment centrifuge cascade safeguards
yawrate pitchrate rollrate spinrate despin yoYo despinWeight
nutationdamper energysink majoraxis minoraxis dualSpinDamping
flatSpin intermediateAxis tennisRacket dzhanibekov polhode herpolhode
bodyCone spaceCone torqueFree eulerPoinsot binet ellipsoidOfInertia
principalAxis productOfInertia parallelAxis perpendicularAxis
radiusOfGyration slenderness buckling eulerLoad crippling
interRivet wrinkling delamination disbond fiberBreak matrixCrack
impactDamage barelyVisible allowable knockdown envirFactor
hygrothermal moistureAbsorption outgassingRate totalMassLoss
collectedVolatiles condensable vented plumeImpingement backflow
contaminationControl witnessPlate quartzMicrobalance residualGas
massSpectrometer ionGauge coldCathode bayardAlpert spinningRotor
heliumLeak snifferProbe bubbleTest pressureDecay rateOfRise
proofPressure burstPressure yieldPressure designBurst safetyFactor
fractureToughness stressIntensity crackTip plasticZone
parislaw threshold toughness ductileToBrittle transitionTemperature
charpy izod impactTest tensileTest coupon specimen strainGauge
extensometer loadCell provingRing deadweight calibrationStandard
traceable nationalInstitute primaryStandard secondaryTransfer
measurementUncertainty coverageFactor expandedUncertainty
repeatability reproducibility intermediatePrecision biasCorrection
"""


CORPUS_EXTRA5 = """
kilometer centimeter millimeter micrometer nanometer kilonewton meganewton
kilojoule megajoule gigajoule kilopascal megapascal millibar microbar
kilogauss terahertz gigahertz megahertz kilohertz arcsecond arcminute
kibibyte mebibyte gibibyte tebibyte baud shannon dyne poise stoke torr
micron reaumur romer slug poundal stone fathom knot stilb phot curie
roentgen svedberg kayser wavenumber diopter neper decibel phon sone
octave semitone cent mohs brinell vickers rockwell shore knoop janka
saybolt engler redwood poiseuille reyn eotvos jansky crab kiloparsec
megaparsec gigaparsec solarmass lunation anomalisticmonth nodalmonth
saros metonic exeligmos inex tritos callippic hipparchic
ephemerisTime dynamicalTime coordinateTime properTime retardedTime
lightCone worldline geodesic nullGeodesic timelike spacelike
interval metric tensor christoffel riemann ricci weyl kretschmann
einsteinHilbert action principle variation extremal eulerLagrange
noether symmetry conservation invariant covariant contravariant
fourVector fourMomentum fourVelocity minkowski metricSignature
tetrads vierbein spinConnection parallelTransport holonomy
curvatureScalar torsionTensor affineConnection leviCivita
gaussBonnet stokesTheorem greensTheorem divergenceTheorem
"""


def candidates() -> list[str]:
    seen_words: set[str] = set()
    out: list[str] = []
    for raw in (
        CORPUS + "\n" + CORPUS_EXTRA + "\n" + CORPUS_EXTRA2 + "\n"
        + CORPUS_EXTRA3 + "\n" + CORPUS_EXTRA4 + "\n" + CORPUS_EXTRA5
    ).split():
        word = raw.strip().lower()
        if not word or word in seen_words:
            continue
        if not (4 <= len(word) <= 10):
            continue
        if not word.isascii() or not word.isalpha():
            continue
        seen_words.add(word)
        out.append(word)
    return out


def select(words: list[str]) -> list[str]:
    prefixes: dict[str, str] = {}
    chosen: list[str] = []
    for word in words:
        prefix = word[:4]
        if prefix in prefixes:
            continue
        prefixes[prefix] = word
        chosen.append(word)
    return sorted(chosen)


def main() -> int:
    words = select(candidates())
    print(f"unique 4-prefix aerospace candidates: {len(words)}")
    if len(words) < TARGET:
        print(f"SHORT by {TARGET - len(words)}; extend CORPUS", file=sys.stderr)
        return 1
    words = words[:TARGET]
    body = "\n".join(f'    "{w}",' for w in words)
    out = ROOT / "crates/odd-identity/src/wordlist.rs"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(
        "//! The 2048-word Aerospace Flight License table.\n"
        "//!\n"
        "//! Generated by `tools/identity-wordlist/build_wordlist.py`; do not edit\n"
        "//! by hand. Words are 4-10 lowercase ASCII letters, sorted for binary\n"
        "//! search, with unique four-letter prefixes so a prefix identifies\n"
        "//! exactly one word.\n\n"
        f"/// Exactly {TARGET} entries, alphabetically sorted.\n"
        f"pub static WORDS: [&str; {TARGET}] = [\n{body}\n];\n\n"
        "/// Index of a word by binary search. Input must already be lowercase.\n"
        "pub fn index_of(word: &str) -> Option<usize> {\n"
        "    WORDS.binary_search(&word).ok()\n"
        "}\n\n"
        "/// Resolve a four-letter (or longer) prefix to its unique word.\n"
        "/// Every word has a distinct four-letter prefix, so this is unambiguous\n"
        "/// and drives autocomplete in the restore dialog.\n"
        "pub fn from_prefix(prefix: &str) -> Option<&'static str> {\n"
        "    if prefix.len() < 4 {\n"
        "        return None;\n"
        "    }\n"
        "    let start = WORDS.partition_point(|w| *w < prefix);\n"
        "    WORDS.get(start).filter(|w| w.starts_with(prefix)).copied()\n"
        "}\n"
    )
    print(f"wrote {out} ({TARGET} words)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
